use glib::prelude::*;

mod connection;
mod imp;
mod server;

glib::wrapper! {
    pub struct RtmpSrvSrc(ObjectSubclass<imp::RtmpSvrSrc>) @extends gst_base::PushSrc, gst_base::BaseSrc, gst::Element, gst::Object;
}

unsafe impl Send for RtmpSrvSrc {}
unsafe impl Sync for RtmpSrvSrc {}

pub fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rtmpsvrsrc",
        gst::Rank::None,
        RtmpSrvSrc::static_type(),
    )?;

    Ok(())
}

gst::plugin_define!(
    rtmpsrvsrc,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
