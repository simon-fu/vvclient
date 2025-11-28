// #[macro_export]
macro_rules! define_error_code {
    ($name:ident, $module_code:expr, {
        $($custom_err:ident = ($custom_code:expr, $custom_desc:expr)),* $(,)?
    }) => {
        #[repr(i32)]
        #[derive(Debug, PartialEq, Eq, Clone, Copy, num_enum::TryFromPrimitive)]
        pub enum $name {
            // 公共错误码
            Success = 0,
            SystemBusy = $module_code * 10000 + 1000,
            InternalError = $module_code * 10000 + 1001,
            ServerConnectionFailed = $module_code * 10000 + 1002,
            IllegalOperationRequest = $module_code * 10000 + 1004,
            UnknownService = $module_code * 10000 + 1010,
            InvalidResponseData = $module_code * 10000 + 1011,
            SystemProcessing = $module_code * 10000 + 1012,
            DataReceived = $module_code * 10000 + 1013,
            FileNotFound = $module_code * 10000 + 1029,
            FileReadError = $module_code * 10000 + 1031,
            FileWriteError = $module_code * 10000 + 1032,
            OperationRestricted = $module_code * 10000 + 1105,
            UserError = $module_code * 10000 + 1106,
            PasswordError = $module_code * 10000 + 1107,
            InvalidToken = $module_code * 10000 + 1108,
            SignatureVerificationError = $module_code * 10000 + 1109,
            RedisError = $module_code * 10000 + 1227,
            MySQLError = $module_code * 10000 + 1228,
            KafkaError = $module_code * 10000 + 1229,
            NetworkConnectionFailed = $module_code * 10000 + 1303,
            ServiceTimeout = $module_code * 10000 + 1405,
            RequiredParameterMissing = $module_code * 10000 + 1506,
            UnifiedRequestParameterError = $module_code * 10000 + 1507,
            // 模块自定义错误码
            $(
                $custom_err = {
                    const _: () = assert!($custom_code >= 1000 && $custom_code <= 9999, "Error code must be a 4-digit number");
                    $module_code * 10000 + $custom_code
                }
            ),*
        }

        impl $name {
            pub fn description(&self) -> &'static str {
                match *self {
                    // 公共错误码说明
                    $name::Success => "Success",
                    $name::SystemBusy => "System is busy",
                    $name::InternalError => "Internal system error",
                    $name::ServerConnectionFailed => "Failed to connect to the server",
                    $name::IllegalOperationRequest => "Illegal operation request",
                    $name::UnknownService => "Unrecognized service",
                    $name::InvalidResponseData => "Invalid response data",
                    $name::SystemProcessing => "System is processing",
                    $name::DataReceived => "Data has been received",
                    $name::FileNotFound => "File not found",
                    $name::FileReadError => "Error reading file",
                    $name::FileWriteError => "Error writing file",
                    $name::OperationRestricted => "Operation restricted",
                    $name::UserError => "User error",
                    $name::PasswordError => "Password error",
                    $name::InvalidToken => "Invalid token",
                    $name::SignatureVerificationError => "Signature verification failed",
                    $name::RedisError => "Redis connection error",
                    $name::MySQLError => "MySQL connection error",
                    $name::KafkaError => "Kafka connection error",
                    $name::NetworkConnectionFailed => "Failed to connect to the network",
                    $name::ServiceTimeout => "Service request timed out",
                    $name::RequiredParameterMissing => "Required parameter is missing",
                    $name::UnifiedRequestParameterError => "Unified request parameter error",

                    // 模块自定义错误码说明
                    $(
                        $name::$custom_err => $custom_desc,
                    )*
                }
            }
        }

        impl From<$name> for $crate::proto::Status {
            fn from(value: $name) -> Self {
                Self {
                    code: value as i32,
                    reason: format!("{value:?}"),
                }
            }
        }

        impl $name {
            pub fn to_status<S: Into<String>>(self, reason: S) -> $crate::proto::Status {
                $crate::proto::Status {
                    code: self as i32,
                    reason: reason.into(),
                }
            }
        }


        impl From<$name> for i32 {
            fn from(value: $name) -> Self {
                value as i32
            }
        }


        pub trait IntoStatus {
            fn into_status(self) -> $crate::proto::Status;
        }

        impl IntoStatus for anyhow::Error {
            fn into_status(self) -> $crate::proto::Status {
                match self.downcast::<$crate::proto::Status>() {
                    Ok(v) => v,
                    Err(e) => {
                        $crate::proto::Status {
                            code: $name::InternalError as i32,
                            reason: format!("internal error: [{e}]"),
                        }
                    },
                }
            }
        }
    };
}


define_error_code!(ErrorCode, 88, {    
    DupUser = (1601, "DupUser"),
    DupSession = (1602, "DupSession"),
    SessionClosed = (1603, "SessionClosed"),
    ConnectionClosed= (1604, "ConnectionClosed"),
    NotFoundSession= (1605, "NotFoundSession"),
    ReconnSession= (1606, "ReconnectSession"),
    RoomEnded = (1607, "RoomEnded"),
});
