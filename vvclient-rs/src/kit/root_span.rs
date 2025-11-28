const DEFAULT_ROOT_NAME: &'static str = "";

pub fn init_root_span(root: tracing::Span) -> tracing::Span {
    
    // let root = tracing::span!(parent: &root, tracing::Level::ERROR, DEFAULT_ROOT_NAME);
    
    inner_span().get_or_init(|| root.into());
    
    
    let root = get_root_span_guard();
    // println!("root span is_none {}", root.as_ref().is_none());
    if root.as_ref().is_none() {
        // Never reach here.
        // These codes only for suppressing warning
        let _r = root_span_child!();
        let _r = root_span_child!("");
        let _r = root_span_child!("", t="");
        let _r = current_span_child!("never");
    }

    root.as_ref().clone()

}

// #[test]
// fn test() {
//     init_root_span(tracing::span!(parent: None, tracing::Level::ERROR, ROOT_NAME));
// }





// #[macro_export]
macro_rules! root_span_child {
    () => {
        $crate::get_root_span()
    };

    ($name:expr) => {
        tracing::span!(parent: $crate::get_root_span_guard().as_ref(), tracing::Level::ERROR, $name)
    };

    ($name:expr, $($fields:tt)*) => {
        tracing::span!(parent: $crate::get_root_span_guard().as_ref(), tracing::Level::ERROR, $name, $($fields)*)
    }
}

pub(crate) use root_span_child; 


// #[macro_export]
macro_rules! current_span_child {

    ($name:expr) => {
        tracing::span!(tracing::Level::ERROR, $name)
    };

    ($name:expr, $($fields:tt)*) => {
        tracing::span!(tracing::Level::ERROR, $name, $($fields)*)
    };

    // ($($args:tt)*) => {
    //     tracing::span!($($args)*)
    // };
}

pub(crate) use current_span_child;


pub(crate) fn get_root_span() -> tracing::Span {
    get_root_span_guard().as_ref().clone()
}


pub(crate) fn get_root_span_guard() -> RootSpanGuard<'static> {
    get_inner_span().guard()
}


fn get_inner_span() -> &'static RootSpan {
    inner_span().get_or_init(||{
        let span = tracing::span!(parent: None, tracing::Level::ERROR, DEFAULT_ROOT_NAME);
        span.into()
    })
}


fn inner_span() -> &'static std::sync::OnceLock<RootSpan> {
    static SPAN: std::sync::OnceLock<RootSpan> = std::sync::OnceLock::new();
    &SPAN
}


pub(crate) struct RootSpanGuard<'a>(&'a tracing::Span);
impl<'a> AsRef<tracing::Span> for RootSpanGuard<'a> {
    fn as_ref(&self) -> &tracing::Span {
        self.0
    }
}

pub(crate) struct RootSpan(tracing::Span);

impl From<tracing::Span> for RootSpan {
    fn from(value: tracing::Span) -> Self {
        Self(value)
    }
}

impl RootSpan {
    pub fn guard<'a>(&'a self) -> RootSpanGuard<'a> {
        RootSpanGuard(&self.0)
    }
}




// struct RootSpan(arc_swap::ArcSwap<tracing::Span>);

// impl RootSpan {
//     pub fn guard(&self) -> RootSpanGuard {
//         self.0.load()
//     }
// }

// impl From<tracing::Span> for RootSpan {
//     fn from(value: tracing::Span) -> Self {
//         Self(arc_swap::ArcSwap::from_pointee(value))
//     }
// }

// type RootSpanGuard<'a> = arc_swap::Guard<std::sync::Arc<tracing::Span>>;



