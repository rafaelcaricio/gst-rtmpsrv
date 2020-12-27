use glib::subclass;
use glib::subclass::prelude::*;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::{gst_debug, gst_error, gst_info, gst_log};
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;

use once_cell::sync::Lazy;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "rtmpsrvsrc",
        gst::DebugColorFlags::empty(),
        Some("RTMP Server Source"),
    )
});

pub struct RtmpSvrSrc {

}

impl ObjectSubclass for RtmpSvrSrc {
    const NAME: &'static str = "RtmpSvrSrc";
    type Type = super::RtmpSrvSrc;
    type ParentType = gst_base::PushSrc;
    type Instance = gst::subclass::ElementInstanceStruct<Self>;
    type Class = subclass::simple::ClassStruct<Self>;

    glib::object_subclass!();

    fn class_init(klass: &mut Self::Class) {
        klass.set_metadata(
            "RTMP Server Source",
            "Source/Video",
            "Creates a server that is capable of receiving a RTMP stream",
            "Rafael Caricio <rafael@caricio.com>",
        );

        let caps = gst::Caps::new_simple(
            "video/x-raw",
            &[],
        );

        let src_pad_template = gst::PadTemplate::new(
            "src",
            gst::PadDirection::Src,
            gst::PadPresence::Always,
            &caps,
        ).unwrap();
        klass.add_pad_template(src_pad_template);
    }

    fn new() -> Self {
        Self {}
    }
}

impl ObjectImpl for RtmpSvrSrc {}
impl ElementImpl for RtmpSvrSrc {}
impl BaseSrcImpl for RtmpSvrSrc {}
impl PushSrcImpl for RtmpSvrSrc {}
