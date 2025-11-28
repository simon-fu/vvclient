

use crate::proto::fmt_writer::JsonDisplay;

use super::UpdateTreeRequest;

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct PacketSer<B> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typ: Option<i32>,  // see PacketType

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sn: Option<i64>,  // seq number

    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<B>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<i64>,  // ack sn
}


#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct ClientRequestSer<T> {
    pub typ: T,
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum OpenTypeSer<T> {
    Open(T),

    // Reconn(super::ReconnectRequest),

    // Close(super::CloseSessionRequest),

    // CreateX(super::CreateWebrtcTransportRequest),

    // ConnX(super::ConnectWebrtcTransportRequest),

    // Pub(super::PublishRequest),

    // UPub(super::UnPublishRequest),

    // Sub(super::SubscribeRequest),

    // USub(super::UnSubscribeRequest),

    // Mute(super::MuteRequest),

    // Layer(super::LayerRequest),

    // End(super::EndRequest),

    // UpExt(super::UpdateExtRequest),

    // UpUTree(super::UpdateTreeRequest),

    // UpRTree(super::UpdateTreeRequest),

    // Chat(super::ChatRequest),

    // Ack(super::PushAck),
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct OpenSessionRequestSer<'a, UTREE> 
// where 
//     // UTREE: Iterator<Item = UpdateTreeRequestRef<'a>> + 'a,
//     UTREE: serde::Serialize,
{
    pub user_id: &'a str,

    pub room_id: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_ext: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_tree: Option< UTREE >,
    // pub user_tree: Option<Vec<UpdateTreeRequestRef<'a>>>,
}

impl<'a, UTREE> OpenSessionRequestSer<'a, UTREE> {
    fn into_msg(self) -> ClientRequestSer<OpenTypeSer<Self>> {
        ClientRequestSer {
            typ: OpenTypeSer::Open(self)
        }
    }

    pub fn into_body(self) -> JsonDisplay<ClientRequestSer<OpenTypeSer<Self>>> {
        JsonDisplay(self.into_msg())
    }
}

// impl<'a, UTREE> OpenSessionRequestSer<'a, UTREE> 
// where 
//     // UTREE: Iterator<Item = UpdateTreeRequestRef<'a>> + 'a,
//     UTREE: serde::Serialize + 'a,
// {
//     pub fn into_msg(self) -> impl serde::Serialize + 'a {
//         ClientRequestRef {
//             typ: OpenTypeRef::Open(self)
//         }
//     }
// }

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct UpdateTreeRequestSer<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune: Option<bool>,
}

impl<'a> From<&'a UpdateTreeRequest> for UpdateTreeRequestSer<'a> {
    fn from(src: &'a UpdateTreeRequest) -> Self {
        Self {
            path: src.path.as_deref(),
            value: src.value.as_deref(),
            prune: src.prune,
        }
    }
}


pub trait IntoIterSerialize {
    fn into_iter_ser(self) -> impl serde::Serialize;
}

impl<I, O> IntoIterSerialize for I 
where 
    I: Iterator<Item = O> + Clone,
    O: serde::Serialize,
{
    fn into_iter_ser(self) -> impl serde::Serialize {
        IterSer(self)
    }
}


#[derive(Clone, PartialEq, Debug)]
pub struct IterSer<I>(I);

impl<I, O> serde::Serialize for IterSer<I> 
where 
    I: Iterator<Item = O> + Clone,
    O: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        use serde::ser::SerializeSeq;

        let iter = self.0.clone();

        // 如果能从迭代器得到 size_hint().1，可传入 Some(len)
        let (lower, upper) = iter.size_hint();
        let mut seq = match upper {
            Some(u) if u == lower => serializer.serialize_seq(Some(u))?,
            Some(u) => serializer.serialize_seq(Some(u))?,
            None => serializer.serialize_seq(None)?,
        };

        for item in iter {
            seq.serialize_element(&item)?;
        }

        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use crate::proto::PacketType;

    use super::*;

    #[test]
    fn test() {
        let user_tree = Some(vec![
            UpdateTreeRequest {
                path: Some("foo/bar".into()), 
                value: Some("value-123".into()),
                prune: None,
            },
            UpdateTreeRequest {
                path: Some("root/child".into()), 
                value: Some("child-xyz".into()),
                prune: Some(true),
            },
        ]);

        let req = OpenSessionRequestSer {
            user_id: "John",
            room_id: "Room1",
            user_ext: Some("ext-abc"),
            user_tree: user_tree
                .as_ref()
                .map(|x|
                    x.iter().map(|y|UpdateTreeRequestSer::from(y))
                    .into_iter_ser()
                ),
        };

        let packet = PacketSer {
            typ: Some(PacketType::Request.as_num()),
            sn: Some(1), 
            body: Some(req.into_body()),
            ack: None, 
        };

        let json = serde_json::to_string(&packet).unwrap();
        println!("{}", json);
        assert_eq!(
            json,
            r#"{"typ":3,"sn":1,"body":"{\"typ\":{\"Open\":{\"user_id\":\"John\",\"room_id\":\"Room1\",\"user_ext\":\"ext-abc\",\"user_tree\":[{\"path\":\"foo/bar\",\"value\":\"value-123\"},{\"path\":\"root/child\",\"value\":\"child-xyz\",\"prune\":true}]}}}"}"#,
        );

    }
}
