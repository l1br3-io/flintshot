//! COSMIC panel applet: panel button + popup menu.
//!
//! This is the Task 7 UI layer. It builds a [`cosmic::Application`] that
//! shows a panel button; clicking it opens a popup mirroring the tray
//! menu — *Capture region*, *Capture fullscreen*, per-monitor entries,
//! *Pin last*, *Open last*, *Open config*, *Quit*. Menu clicks are
//! resolved by the pure [`crate::menu`] dispatch (unit-tested headless)
//! and performed here as side-effects: daemon [`Request`]s go out via
//! [`crate::ipc::send_request`], and *Open config* shells out to
//! `$EDITOR`.
//!
//! The applet lifecycle (popup open/close, surface actions) follows
//! libcosmic's canonical applet template — see the `applet` example in
//! the libcosmic tree. Task 8 layers the live event stream + daemon-
//! absent status on top of this shell.

use cosmic::app::{Core, Task};
use cosmic::iced::core::window;
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Rectangle, Vector};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{divider, text};
use cosmic::{applet, Action as CosmicAction, Element};

use crate::menu::{self, MenuMessage};

/// Reverse-DNS app id (COSMIC/xdg convention).
const APP_ID: &str = "dev.flintshot.FlintshotCosmic";

/// Freedesktop icon name for the panel button. `camera-photo` is widely
/// installed; COSMIC falls back gracefully if it's missing. The polish
/// sweep can swap in a branded mark.
const PANEL_ICON: &str = "camera-photo";

/// The applet state: a COSMIC [`Core`] plus the current popup id (if
/// the popup is open). The panel button toggles `popup`.
pub struct App {
    core: Core,
    popup: Option<Id>,
}

/// Messages handled by [`App`]. Menu actions ([`Message::Menu`]) are
/// forwarded to the pure [`menu::action_for`] resolver; the rest is
/// libcosmic lifecycle (popup close, surface action, async completion).
#[derive(Clone, Debug)]
pub enum Message {
    /// A popup menu item was clicked.
    Menu(MenuMessage),
    /// A popup window requested close (libcosmic lifecycle).
    PopupClosed(Id),
    /// A libcosmic surface action (popup open/close plumbing).
    Surface(cosmic::surface::Action),
    /// An async IPC send completed (fire-and-forget; errors are logged
    /// in the future before this is emitted).
    Sent,
}

impl cosmic::Application for App {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        (Self { core, popup: None }, Task::none())
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Menu(menu_msg) => match menu::action_for(&menu_msg) {
                menu::Action::Request(req) => {
                    // Fire-and-forget: clone the Request, send it on the
                    // runtime, log failures, yield a no-op when done.
                    // The socket-path override is None → platform default.
                    return cosmic::task::future(async move {
                        if let Err(e) = crate::ipc::send_request(None, &req).await {
                            tracing::warn!(error = %e, "applet: sending IPC request");
                        }
                        Message::Sent
                    });
                }
                menu::Action::OpenConfig => {
                    if let Err(e) = menu::open_config() {
                        tracing::warn!(error = %e, "applet: opening config");
                    }
                }
            },
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::Surface(a) => {
                return cosmic::task::message(CosmicAction::Cosmic(cosmic::app::Action::Surface(
                    a,
                )));
            }
            Message::Sent => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let have_popup = self.popup;
        let btn = self
            .core
            .applet
            .icon_button(PANEL_ICON)
            .on_press_with_rectangle(move |offset: Vector, bounds: Rectangle| {
                if let Some(id) = have_popup {
                    Message::Surface(destroy_popup(id))
                } else {
                    Message::Surface(app_popup::<App>(
                        |_| cosmic::surface::action::LiveSettings::default(),
                        move |state: &mut App| {
                            let new_id = Id::unique();
                            state.popup = Some(new_id);
                            let mut popup_settings = state.core.applet.get_popup_settings(
                                state.core.main_window_id().unwrap_or(Id::RESERVED),
                                new_id,
                                None,
                                None,
                                None,
                            );
                            popup_settings.positioner.anchor_rect =
                                rect_from_bounds(offset, bounds);
                            popup_settings
                        },
                        Some(Box::new(move |state: &App| {
                            state.popup_content().map(CosmicAction::App)
                        })),
                    ))
                }
            });

        Element::from(self.core.applet.applet_tooltip::<Message>(
            btn,
            "flintshot",
            self.popup.is_some(),
            Message::Surface,
            None,
        ))
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        self.popup_content()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(applet::style())
    }
}

impl App {
    /// Build the popup body: a column of menu buttons with dividers
    /// between the capture / monitor / pin / system groups. Mirrors the
    /// tray's menu structure item-for-item; each button emits a
    /// [`MenuMessage`] that [`menu::action_for`] resolves.
    fn popup_content(&self) -> Element<'_, Message> {
        let monitors = menu::monitor_entries();

        let mut col = cosmic::widget::Column::new()
            .push(menu_btn("Capture region", MenuMessage::CaptureRegion))
            .push(menu_btn(
                "Capture fullscreen",
                MenuMessage::CaptureFullscreen,
            ))
            .push(divider::horizontal::default());

        if monitors.is_empty() {
            col = col.push(disabled_item("no monitors"));
        } else {
            for (index, label) in monitors {
                col = col.push(menu_btn(
                    label,
                    MenuMessage::CaptureFullscreenMonitor(index),
                ));
            }
        }

        col = col
            .push(divider::horizontal::default())
            .push(menu_btn("Pin last", MenuMessage::PinLast))
            .push(menu_btn("Open last", MenuMessage::OpenLast))
            .push(divider::horizontal::default())
            .push(menu_btn("Open config", MenuMessage::OpenConfig))
            .push(menu_btn("Quit", MenuMessage::Quit))
            .width(Length::Fill);

        Element::from(self.core.applet.popup_container(col))
    }
}

/// A single clickable popup menu row: `menu_button`-styled, presses emit
/// `msg`. Uses `applet::menu_button` for the COSMIC popup look.
fn menu_btn<'a>(label: impl Into<String>, msg: MenuMessage) -> Element<'a, Message> {
    applet::menu_button(text(label.into()))
        .on_press(Message::Menu(msg))
        .into()
}

/// A disabled (non-interactive) menu row, used for the "no monitors"
/// placeholder. Visually matches [`menu_btn`] but produces no message.
fn disabled_item<'a>(label: impl Into<String>) -> Element<'a, Message> {
    applet::padded_control(text(label.into())).into()
}

/// Build the popup's `anchor_rect` from the button's offset + bounds.
/// `bounds` is `Rectangle<f32>` (iced default); the positioner wants
/// `Rectangle<i32>`. Screen coordinates are small positive f32s (low
/// thousands at most), so truncation to i32 is lossless in practice —
/// mirrors libcosmic's own applet examples.
#[allow(clippy::cast_possible_truncation)]
fn rect_from_bounds(offset: Vector, bounds: Rectangle) -> cosmic::iced::Rectangle<i32> {
    cosmic::iced::Rectangle {
        x: (bounds.x - offset.x) as i32,
        y: (bounds.y - offset.y) as i32,
        width: bounds.width as i32,
        height: bounds.height as i32,
    }
}
