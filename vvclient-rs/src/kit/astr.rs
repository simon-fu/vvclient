
pub use arc_str::AStr;


// #[cfg(debug_assertions)]
// pub type AStr = String ;

// // #[cfg(not(debug_assertions))]
// // pub type AStr = std::sync::Arc<str>;

// #[cfg(not(debug_assertions))]
// pub use arc_str::AStr;

// #[cfg(not(debug_assertions))]
mod arc_str {
    use core::{fmt, hash};
    use std::{borrow::Borrow, ops::Deref, sync::Arc};

    #[cfg(not(debug_assertions))]
    #[derive(Clone)]
    pub struct AStr(Arc<str>);

    #[cfg(debug_assertions)]
    #[derive(Clone)]
    pub struct AStr(Arc<String>);

    impl AStr {
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl fmt::Debug for AStr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            fmt::Debug::fmt(self.as_str(), f)
        }
    }

    impl fmt::Display for AStr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            fmt::Display::fmt(self.as_str(), f)
        }
    }


    impl Deref for AStr {
        type Target = str;

        fn deref(&self) -> &Self::Target {
            self.as_str()
        }
    }

    impl Into<String> for AStr {
        fn into(self) -> String {
            self.as_str().into()
        }
    }

    impl From<String> for AStr {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }

    impl From<&String> for AStr {
        fn from(value: &String) -> Self {
            Self(value.clone().into())
        }
    }

    #[cfg(not(debug_assertions))]
    impl<'a> From<&'a str> for AStr {
        fn from(value: &'a str) -> Self {
            Self(value.into())
        }
    }

    #[cfg(debug_assertions)]
    impl<'a> From<&'a str> for AStr {
        fn from(value: &'a str) -> Self {
            Self(Arc::new(value.into()))
        }
    }

    impl hash::Hash for AStr {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.as_str().hash(state);
        }
    }

    impl Eq for AStr {

    }

    impl PartialEq for AStr {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl PartialEq<String> for AStr {
        fn eq(&self, other: &String) -> bool {
            self.as_str() == other
        }
    }

    impl PartialEq<&str> for AStr {
        fn eq(&self, other: &&str) -> bool {
            self.as_str() == *other
        }
    }

    impl PartialEq<&str> for &AStr {
        fn eq(&self, other: &&str) -> bool {
            self.as_str() == *other
        }
    }

    impl PartialEq<AStr> for &str {
        fn eq(&self, other: &AStr) -> bool {
            *self == other.as_str()
        }
    }

    impl PartialEq<&AStr> for &str {
        fn eq(&self, other: &&AStr) -> bool {
            *self == other.as_str()
        }
    }

    impl Borrow<str> for AStr {
        fn borrow(&self) -> &str {
            self.as_str()
        }
    }

    impl serde::Serialize for AStr {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
            serializer.serialize_str(self.as_str())
        }
    }

    impl<'de> serde::Deserialize<'de> for AStr {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            Ok(s.into())
        }
    }

}

