use std::{path::PathBuf, sync::Arc};

use editor::{Editor, ToPoint};
use gpui::{
    actions, prelude::*, AppContext, DismissEvent, EventEmitter, FocusHandle, FocusableView,
    KeyBinding, View, WeakView,
};
use log::info;
use project::Bookmark;
use ui::{div, h_flex, rems, v_flex, ActiveTheme, StyledExt, ViewContext, WindowContext};
use workspace::{item::ItemHandle, ModalView, Workspace};

actions!(annotation, [Confirm]);

// FIXME: code duplication
fn create_editor(placeholder: Arc<str>, cx: &mut WindowContext<'_>) -> View<Editor> {
    cx.new_view(|cx| {
        let mut editor = Editor::single_line(cx);
        editor.set_placeholder_text(placeholder, cx);
        editor
    })
}

pub struct Annotation {
    editor: View<Editor>,
}

impl Render for Annotation {
    fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
        self.editor.clone()
    }
}

pub struct AnnotationView {
    editor: View<Editor>,
    workspace: WeakView<Workspace>,
}

impl ModalView for AnnotationView {}
impl EventEmitter<DismissEvent> for AnnotationView {}

impl AnnotationView {
    fn new(cx: &mut ViewContext<Self>, workspace: WeakView<Workspace>) -> Self {
        cx.bind_keys([KeyBinding::new("enter", Confirm, None)]);

        Self {
            editor: create_editor(Arc::from("Add Bookmark..."), cx),
            workspace,
        }
    }

    pub fn cancel(&mut self, _: &menu::Cancel, cx: &mut ViewContext<Self>) {
        cx.emit(DismissEvent);
    }

    fn confirm(&mut self, _: &Confirm, cx: &mut ViewContext<Self>) {
        let bookmark_label = self.editor.read(cx).text(cx);
        info!("New Bookmark with Annotation {}", bookmark_label);

        if let Some(workspace) = self.workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                let project = workspace.project();
                if let Some(editor) = workspace.active_item_as::<Editor>(cx) {
                    let point = editor.update(cx, |editor, cx| {
                        let snapshot = editor.snapshot(cx).display_snapshot.buffer_snapshot;
                        let cursor_position = editor.selections.newest_anchor().head();
                        let point = cursor_position.to_point(&snapshot);
                        point
                    });

                    if let Some(path) = editor.project_path(cx) {
                        info!(
                            "Adding new bookmark with path {} for line {}",
                            path.path.to_string_lossy(),
                            point.row
                        );

                        project.update(cx, |project, cx| {
                            project.bookmarks_mut().update(cx, |bookmarks, _cx| {
                                bookmarks.push(Bookmark::new(
                                    &bookmark_label,
                                    path,
                                    // TODO: add absolute path
                                    PathBuf::from("/tmp/tmp.rs"),
                                    point,
                                ));
                            })
                        });
                    }
                    cx.notify();
                }
            });
        }

        cx.emit(DismissEvent);
    }

    pub fn open(workspace: &mut Workspace, cx: &mut ViewContext<Workspace>) {
        let weak_workspace = cx.view().downgrade();
        workspace.toggle_modal(cx, |cx| {
            let view = AnnotationView::new(cx, weak_workspace);
            view
        });
    }
}

impl FocusableView for AnnotationView {
    fn focus_handle(&self, cx: &AppContext) -> FocusHandle {
        self.editor.focus_handle(cx)
    }
}

impl Render for AnnotationView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let editor = h_flex()
            .overflow_hidden()
            .flex_none()
            .h_9()
            .px_4()
            .child(self.editor.clone());

        let contents = div()
            .size_full()
            .overflow_hidden()
            .elevation_3(cx)
            .child(editor);

        div()
            .flex()
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::confirm))
            .bg(cx.theme().colors().editor_background)
            .size_full()
            .justify_center()
            .items_center()
            .child(v_flex().w(rems(34.)).child(contents))
    }
}
