//! Public links shown in the app (About / support). Edit these in one place.
pub const MAKER: &str = "Chris & Stacy Weston, Backwoods Fabrications";
pub const YOUTUBE_URL: &str = "https://www.youtube.com/@backwoodsfab";
pub const FACEBOOK_URL: &str = "https://www.facebook.com/westonsbackwoods/";
pub const DONATE_URL: &str = "https://www.paypal.com/ncp/payment/G9SEGNDK2RYVC";
pub const WEBSITE_URL: &str = "https://www.backwoodsfab.com";

/// The thank-you note shown once on first launch and from the "Our story" button.
pub const STORY_TITLE: &str = "Thank you for giving our application a try!";
pub const STORY: &[&str] = &[
    "At Backwoods Fabrications, we believe innovation starts with curiosity, a willingness to challenge conventional thinking, and the desire to build something better. Whether we're designing custom automotive parts, bringing new life to forgotten classics, or developing software like this, our approach remains the same \u{2014} solve real problems, simplify the complicated, and make things possible that once seemed out of reach.",
    "As classic car owners and enthusiasts ourselves, many of the products and ideas we develop begin with a need for our own vehicles. We're passionate about preserving the heritage, history, and individuality of classic and antique automobiles, keeping the spirit of these iconic brands alive for generations to come.",
    "This application was born from a real-world project in our own Mid-Michigan garage. Rather than simply solving a problem for ourselves, we saw an opportunity to create something that could benefit the entire maker community.",
    "If you find value in what we've created, consider supporting our efforts through a donation, following our projects, or simply sharing what we do with others. Every bit of support helps fuel the next idea, experiment, and innovation.",
    "Thank you for being a part of our journey.",
];
pub const STORY_SIGN: &str = "Chris & Stacy Weston";
pub const STORY_TAG: &str = "Built from experience. Driven by curiosity. Inspired by what's possible.";

/// Opens a web address in the person's default browser (Mac `open`, Windows file handler, Linux `xdg-open`).
/// Only http(s) addresses are accepted. Failures are ignored: there is nothing useful to show.
pub fn open(url: &str) {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return;
    }
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    let _ = r;
}
