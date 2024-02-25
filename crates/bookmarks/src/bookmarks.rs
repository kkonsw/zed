use annotation::AnnotationView;
use editor::{scroll::Autoscroll, Editor};
use fuzzy::{StringMatch, StringMatchCandidate};
use gpui::{
    actions, AnyElement, AppContext, DismissEvent, EventEmitter, FocusHandle, FocusableView, Model,
    Task, View, WeakView,
};
use log::info;
use ordered_float::OrderedFloat;
use picker::{Picker, PickerDelegate};
use project::Project;
use std::sync::Arc;
use text::Bias;
use ui::{prelude::*, HighlightedLabel, ListItem, ListItemSpacing, Tooltip};
use util::ResultExt;
use workspace::{ModalView, Workspace};

mod annotation;

pub fn init(cx: &mut AppContext) {
    cx.observe_new_views(BookmarkView::register).detach();
}

pub struct BookmarkView {
    picker: View<Picker<BookmarkViewDelegate>>,
}

actions!(bookmarks, [Toggle, AddBookmark]);

impl EventEmitter<DismissEvent> for BookmarkView {}

impl FocusableView for BookmarkView {
    fn focus_handle(&self, cx: &AppContext) -> FocusHandle {
        self.picker.focus_handle(cx)
    }
}

impl ModalView for BookmarkView {}

impl BookmarkView {
    fn register(workspace: &mut Workspace, _: &mut ViewContext<Workspace>) {
        workspace.register_action(|workspace, _: &Toggle, cx| {
            let Some(bookmarks) = workspace.active_modal::<Self>(cx) else {
                Self::open(workspace, cx);
                return;
            };

            bookmarks.update(cx, |bookmarks, cx| {
                bookmarks
                    .picker
                    .update(cx, |picker, cx| picker.cycle_selection(cx))
            });
        });

        workspace.register_action(|workspace, _: &AddBookmark, cx| {
            AnnotationView::open(workspace, cx);
        });
    }

    fn open(workspace: &mut Workspace, cx: &mut ViewContext<Workspace>) {
        let weak_workspace = cx.view().downgrade();
        let project = workspace.project().clone();
        workspace.toggle_modal(cx, |cx| {
            let delegate =
                BookmarkViewDelegate::new(cx.view().downgrade(), weak_workspace, project);
            BookmarkView::new(delegate, cx)
        });
    }

    fn new(delegate: BookmarkViewDelegate, cx: &mut ViewContext<Self>) -> Self {
        Self {
            picker: cx.new_view(|cx| Picker::uniform_list(delegate, cx)),
        }
    }
}

impl Render for BookmarkView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .flex()
            .bg(cx.theme().colors().editor_background)
            .size_full()
            .justify_center()
            .items_center()
            .child(v_flex().w(rems(34.)).child(self.picker.clone()))
    }
}

struct BookmarkViewDelegate {
    view: WeakView<BookmarkView>,
    workspace: WeakView<Workspace>,
    project: Model<Project>,
    matches: Vec<StringMatch>,
    selected_index: usize,
}

impl BookmarkViewDelegate {
    fn new(
        view: WeakView<BookmarkView>,
        workspace: WeakView<Workspace>,
        project: Model<Project>,
    ) -> Self {
        Self {
            view,
            workspace,
            project,
            matches: Vec::new(),
            selected_index: 0,
        }
    }

    fn delete_bookmark(&self, ix: usize, cx: &mut ViewContext<Picker<Self>>) {
        let bookmarks = self.project.read(cx).bookmarks().read(cx);
        let bookmark = &bookmarks[ix];

        info!("Deleting bookmark {}", bookmark.label());
        if let Some(workspace) = self.workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                let project = workspace.project();
                project.update(cx, |project, cx| {
                    project.bookmarks_mut().update(cx, |bookmarks, _cx| {
                        // FIXME: inefficient remove
                        bookmarks.remove(ix);
                    })
                });
            });

            cx.spawn(move |this, mut cx| async move {
                this.update(&mut cx, move |picker, cx| {
                    picker.delegate.set_selected_index(ix - 1, cx);
                    picker.update_matches(picker.query(cx), cx)
                })
            })
            .detach();
        }
    }
}

impl PickerDelegate for BookmarkViewDelegate {
    type ListItem = ListItem;

    fn placeholder_text(&self, _cx: &mut WindowContext) -> Arc<str> {
        "Search bookmarks...".into()
    }

    fn match_count(&self) -> usize {
        self.matches.len()
    }

    fn selected_index(&self) -> usize {
        self.selected_index
    }

    fn set_selected_index(
        &mut self,
        ix: usize,
        cx: &mut ViewContext<Picker<BookmarkViewDelegate>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn update_matches(
        &mut self,
        query: String,
        cx: &mut ViewContext<Picker<BookmarkViewDelegate>>,
    ) -> Task<()> {
        let query = query.trim_start();
        let smart_case = query.chars().any(|c| c.is_uppercase());

        let candidates = self
            .project
            .read(cx)
            .bookmarks()
            .read(cx)
            .iter()
            .enumerate()
            .map(|(id, bookmark)| StringMatchCandidate::new(id, bookmark.label().clone()))
            .collect::<Vec<_>>();

        self.matches = smol::block_on(fuzzy::match_strings(
            candidates.as_slice(),
            query,
            smart_case,
            100,
            &Default::default(),
            cx.background_executor().clone(),
        ));
        self.matches.sort_unstable_by_key(|m| m.candidate_id);

        self.selected_index = self
            .matches
            .iter()
            .enumerate()
            .rev()
            .max_by_key(|(_, m)| OrderedFloat(m.score))
            .map(|(ix, _)| ix)
            .unwrap_or(0);

        Task::ready(())
    }

    fn confirm(&mut self, _: bool, cx: &mut ViewContext<Picker<BookmarkViewDelegate>>) {
        if let Some(m) = self.matches.get(self.selected_index()) {
            if let Some(workspace) = self.workspace.upgrade() {
                // FIXME: clone
                let bookmark = self.project.read(cx).bookmarks().read(cx)[m.candidate_id].clone();
                let open_task = workspace.update(cx, |workspace, cx| {
                    workspace.open_path(bookmark.project_path().clone(), None, true, cx)
                });

                let view = self.view.clone();
                cx.spawn(|_, mut cx| async move {
                    let item = open_task.await.log_err()?;

                    // Scroll
                    if let Some(active_editor) = item.downcast::<Editor>() {
                        active_editor
                            .downgrade()
                            .update(&mut cx, |editor, cx| {
                                let snapshot = editor.snapshot(cx).display_snapshot;
                                let point = snapshot
                                    .buffer_snapshot
                                    .clip_point(bookmark.point(), Bias::Left);
                                editor.change_selections(Some(Autoscroll::center()), cx, |s| {
                                    s.select_ranges([point..point])
                                });
                            })
                            .log_err();
                    }

                    view.update(&mut cx, |_, cx| cx.emit(DismissEvent)).ok()?;
                    Some(())
                })
                .detach();
            }
        }
    }

    fn dismissed(&mut self, cx: &mut ViewContext<Picker<BookmarkViewDelegate>>) {
        self.view
            .update(cx, |_, cx| cx.emit(DismissEvent))
            .log_err();
    }

    fn render_match(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut ViewContext<Picker<Self>>,
    ) -> Option<Self::ListItem> {
        let candidate = &self.matches[ix];
        let bookmarks = self.project.read(cx).bookmarks().read(cx);

        if candidate.candidate_id >= bookmarks.len() {
            return None;
        }

        let bookmark = &bookmarks[candidate.candidate_id];
        let path = Arc::clone(&bookmark.project_path().path);

        Some(
            ListItem::new(ix)
                .spacing(ListItemSpacing::Sparse)
                .inset(true)
                .selected(selected)
                .child(
                    h_flex()
                        .gap_2()
                        // FIXME: clone, highlighting
                        .child(HighlightedLabel::new(bookmark.label().clone(), Vec::new()))
                        .child(
                            // FIXME: clone, highlighting
                            HighlightedLabel::new(String::from(path.to_string_lossy()), Vec::new())
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                )
                .when(true, |el| {
                    let delete_button = div()
                        .child(
                            IconButton::new("delete", IconName::Close)
                                .icon_size(IconSize::Small)
                                .on_click(cx.listener(move |this, _event, cx| {
                                    cx.stop_propagation();
                                    cx.prevent_default();

                                    this.delegate.delete_bookmark(ix, cx)
                                }))
                                .tooltip(|cx| Tooltip::text("Delete Bookmark...", cx)),
                        )
                        .into_any_element();

                    if self.selected_index() == ix {
                        el.end_slot::<AnyElement>(delete_button)
                    } else {
                        el.end_hover_slot::<AnyElement>(delete_button)
                    }
                }),
        )
    }
}

#[cfg(test)]
mod tests {}
