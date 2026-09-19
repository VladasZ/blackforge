//! The root view: the sidebar, the page next to it, the status bar under the
//! page and the toasts over everything.

use hilen::{
    refs::Weak,
    ui::{Container, Setup, UIEvents, UIManager, View, ViewData, ViewSubviews, view},
};

use crate::ui::{
    browse_page::BrowsePage,
    colors,
    configs_page::ConfigsPage,
    doctor_page::DoctorPage,
    game_page::GamePage,
    mods_page::ModsPage,
    page::Page,
    share_page::SharePage,
    sidebar::{self, Sidebar},
    status::{self, StatusBar},
    toast::{self, ToastHost},
};

#[view]
pub struct Shell {
    page: Option<Weak<dyn View>>,
    toasts: Weak<ToastHost>,

    #[init]
    sidebar: Sidebar,
    content: Container,
    status: StatusBar,
}

impl Setup for Shell {
    fn setup(mut self: Weak<Self>) {
        UIManager::set_clear_color(colors::BG.resolve());
        UIEvents::theme_changed().sub(self, || {
            UIManager::set_clear_color(colors::BG.resolve());
        });

        self.sidebar.place().l(0).t(0).b(0).w(sidebar::WIDTH);
        self.sidebar.selected.val(move |page| self.show(page));

        self.status
            .place()
            .l(sidebar::WIDTH)
            .r(0)
            .b(0)
            .h(status::HEIGHT);
        self.content
            .place()
            .l(sidebar::WIDTH)
            .r(0)
            .t(0)
            .b(status::HEIGHT);

        self.toasts = self.add_subview(toast::host());
        self.toasts.place().l(sidebar::WIDTH).r(0).t(0).b(0);
        toast::register(self.toasts);

        self.show(Page::Mods);
    }
}

impl Shell {
    fn show(mut self: Weak<Self>, page: Page) {
        self.sidebar.select(page);

        if let Some(mut old) = self.page.take() {
            old.remove_from_superview();
        }

        let view: Weak<dyn View> = match page {
            Page::Mods => self.content.add_view::<ModsPage>(),
            Page::Browse => self.content.add_view::<BrowsePage>(),
            Page::Configs => self.content.add_view::<ConfigsPage>(),
            Page::Share => self.content.add_view::<SharePage>(),
            Page::Game => self.content.add_view::<GamePage>(),
            Page::Doctor => self.content.add_view::<DoctorPage>(),
        };
        view.place().back();
        self.page = Some(view);
    }
}
