//! What the Friends page shows while nobody is signed in: what an account
//! gives, and the Google button.

use hilen::{
    Event,
    login::GoogleLoginButton,
    refs::Weak,
    ui::{ImageView, Label, Setup, ViewData, view},
};

use crate::ui::{style, toast};

const PAD: f32 = 20.0;
const TITLE_T: f32 = 18.0;
const LINES_T: f32 = 54.0;
const LINE_H: f32 = 28.0;
const ICON: f32 = 16.0;
const TEXT_L: f32 = PAD + ICON + 10.0;
const LOGIN_T: f32 = LINES_T + 3.0 * LINE_H + 14.0;
const LOGIN_W: f32 = 240.0;
const LOGIN_H: f32 = 40.0;

pub const HEIGHT: f32 = LOGIN_T + LOGIN_H + PAD;

const LINES: [&str; 3] = [
    "See the mods your friends play with",
    "Copy their config settings into yours",
    "Keep your setup the same on every machine",
];

#[view]
pub struct SignInCard {
    pub signed_in: Event,

    #[init]
    title: Label,
    icon_1: ImageView,
    line_1: Label,
    icon_2: ImageView,
    line_2: Label,
    icon_3: ImageView,
    line_3: Label,
    login: GoogleLoginButton,
}

impl Setup for SignInCard {
    fn setup(self: Weak<Self>) {
        style::card(self);

        style::body(self.title);
        self.title.set_text("Sign in to play together");
        self.title.set_text_size(16);
        self.title.place().t(TITLE_T).l(PAD).r(PAD).h(22);

        let icons = [self.icon_1, self.icon_2, self.icon_3];
        let lines = [self.line_1, self.line_2, self.line_3];
        let mut top = LINES_T;
        for ((icon, line), text) in icons.into_iter().zip(lines).zip(LINES) {
            icon.set_image("check.svg");
            icon.place()
                .t(top + (LINE_H - ICON) / 2.0)
                .l(PAD + line.text_inset())
                .size(ICON, ICON);
            style::body(line);
            line.set_text(text);
            line.place().t(top).l(TEXT_L).r(PAD).h(LINE_H);
            top += LINE_H;
        }

        self.login
            .place()
            .t(LOGIN_T)
            .l(PAD + self.title.text_inset())
            .size(LOGIN_W, LOGIN_H);
        self.login
            .logged_in
            .val(move |_| self.signed_in.trigger(()));
        self.login.failed.val(toast::error);
    }
}
