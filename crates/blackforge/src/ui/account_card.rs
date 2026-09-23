//! The account card of the Settings page: who is signed in, with the way to
//! sign out, or the Google button while nobody is.

use hilen::{
    login::GoogleLoginButton,
    refs::Weak,
    ui::{Button, Label, Setup, ViewData, view},
};

use crate::{
    backend, social,
    ui::{style, toast},
};

pub const HEIGHT: f32 = 76.0;

const PAD: f32 = 16.0;
const LOGIN_W: f32 = 220.0;
const SIGN_OUT_W: f32 = 90.0;

#[view]
pub struct AccountCard {
    #[init]
    title: Label,
    status: Label,
    login: GoogleLoginButton,
    sign_out: Button,
}

impl Setup for AccountCard {
    fn setup(self: Weak<Self>) {
        style::card(self);

        style::body(self.title);
        self.title.set_text("Account");
        self.title
            .place()
            .t(PAD)
            .l(PAD)
            .r(PAD + LOGIN_W + PAD)
            .h(20);

        style::dim(self.status);
        self.status.set_ellipsize(true);
        self.status
            .place()
            .t(PAD + 24.0)
            .l(PAD)
            .r(PAD + LOGIN_W + PAD)
            .h(16);

        self.login.place().r(PAD).center_y().size(LOGIN_W, 36);
        self.login.logged_in.val(move |_| {
            social::share_profile();
            self.show();
        });
        self.login.failed.val(toast::error);

        style::ghost(self.sign_out, "Sign out");
        self.sign_out
            .place()
            .r(PAD)
            .center_y()
            .size(SIGN_OUT_W, style::BUTTON_H);
        self.sign_out.on_tap(move || {
            social::sign_out(move || {
                if self.is_ok() {
                    self.show();
                }
            });
        });

        self.show();
    }
}

impl AccountCard {
    fn show(self: Weak<Self>) {
        let signed_in = social::signed_in();
        self.login.set_hidden(signed_in);
        self.sign_out.set_hidden(!signed_in);
        if !signed_in {
            self.status.set_text("Not signed in");
            return;
        }

        self.status.set_text("Signed in");
        backend::load(
            "loading your account",
            |_, _| async move { Ok(social::client()?.me().await?) },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(me) => {
                        self.status.set_text(match me.username {
                            Some(name) => format!("Signed in as {name}"),
                            None => "Signed in, pick a username on the Friends page".to_owned(),
                        });
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }
}
