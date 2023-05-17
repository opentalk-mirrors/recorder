use glib::Cast;
use gst::{traits::GstObjectExt, DebugGraphDetails};

pub const DOT_OUTPUT_PATH: &str = "./pipelines";

pub struct Params {
    pub details: DebugGraphDetails,
    pub index: bool,
}

impl Params {
    pub const fn all() -> Self {
        Self {
            details: DebugGraphDetails::ALL,
            index: true,
        }
    }
    pub const fn states() -> Self {
        Self {
            details: DebugGraphDetails::STATES,
            index: true,
        }
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            details: DebugGraphDetails::ALL,
            index: true,
        }
    }
}

pub fn dot(bin: &impl glib::IsA<gst::Element>, filename_without_extension: &str) {
    dot_ext(bin, filename_without_extension, &Default::default());
}

pub fn dot_ext(
    bin: &impl glib::IsA<gst::Element>,
    filename_without_extension: &str,
    params: &Params,
) {
    // count calls
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNT: AtomicUsize = AtomicUsize::new(0);

    // check if env var 'GST_DEBUG_DUMP_DOT_DIR' has been set properly
    let path = std::env::var("GST_DEBUG_DUMP_DOT_DIR").unwrap_or_else( |_| {
        if COUNT.load(Ordering::SeqCst) == 0 {
            warn!("Using default dod path. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to an absolute path to get DOT output.");
        };
        DOT_OUTPUT_PATH.to_string()
    });

    std::fs::create_dir_all(path.clone()).expect("can not create dir from GST_DEBUG_DUMP_DOT_DIR");
    // find the parent
    match bin.clone().dynamic_cast::<gst::Object>().unwrap().parent() {
        Some(parent) => dot(
            &parent.dynamic_cast::<gst::Element>().unwrap(),
            filename_without_extension,
        ),
        None => {
            let name = if params.index {
                let n = COUNT.fetch_add(1, Ordering::SeqCst);
                let r = format!("{n}-{filename_without_extension}");
                r
            } else {
                filename_without_extension.to_string()
            };

            info!("GENERATING DOT FILE: '{path}/{name}.dot'");

            gst::debug_bin_to_dot_file(
                &Cast::dynamic_cast::<gst::Bin>(bin.clone()).unwrap(),
                params.details,
                name,
            );
        }
    }
}

pub fn name(object: &impl glib::IsA<gst::Object>) -> glib::GString {
    if let Some(parent) = object.parent() {
        format!("{}.{}", name(&parent), object.name()).into()
    } else {
        object.name()
    }
}
