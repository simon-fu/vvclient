use std::collections::VecDeque;

use crate::proto::{IdRef, PacketRef, PacketType, ServerPushRef, ServerResponseRef, Status};

use anyhow::Result;

use log::debug;

type Packet = String;

pub struct Xfer {
    que: VecDeque<(i64, Packet)>,
    tail_sn: i64,
    send_index: usize,
    // sent_sn: i64,
    // recv_ack_sn: i64,

    recv_sn: i64,
    sent_ack_sn: i64,
}

impl Xfer {
    pub fn new() -> Self {
        Self {
            que: Default::default(),
            tail_sn: 0.into(),
            send_index: 0,
            // sent_sn: 0.into(),
            // recv_ack_sn: 0.into(),
            recv_sn: 0.into(),
            sent_ack_sn: 0.into(),
        }
    }

    pub fn is_full(&self) -> bool {
        self.que.len() >= 64
    }

    pub fn init_recv_ack(&mut self, sn: i64) {
        self.update_recv_ack(sn);
        self.send_index = 0;
    }

    pub fn update_recv_ack(&mut self, sn: i64) {
        while let Some(item) = self.que.front() {
            if item.0 > sn {
                break;
            } else {
                self.que.pop_front();
                self.send_index -= 1;
            }
        }
    }

    pub fn update_recv_sn(&mut self, sn: i64) {
        if sn > self.recv_sn {
            self.recv_sn = sn;
        }
    }

    pub fn clear_send_ack(&mut self) {
        self.sent_ack_sn = 0;
    }

    pub fn add_qos1_typed<B>(&mut self, typ: PacketType, body: &B, msg_id: Option<i64>) -> Result<i64>
    where 
        B: serde::Serialize,
    {
        let body_json = serde_json::to_string(body)?;
        
        self.add_qos1_json(typ, &body_json, msg_id)
    }

    pub fn add_qos1_json(&mut self, typ: PacketType, body_json: &str, msg_id: Option<i64>) -> Result<i64> {
        let sn = self.tail_sn + 1;

        let mut packet = PacketRef::qos1(sn, typ, body_json);
        self.fill_ack(msg_id, &mut packet);

        let packet_json = serde_json::to_string(&packet)?;

        // debug!("added qos1 [{}]", packet_json);
        
        self.que.push_back((sn, packet_json));
        self.tail_sn = sn;
        Ok(sn)
    }

    fn fill_ack(&mut self, msg_id: Option<i64>, packet: &mut PacketRef) {
        match msg_id {
            Some(ack) => {
                packet.ack = Some(ack);
                if self.sent_ack_sn < ack {
                    self.sent_ack_sn = ack;
                }
            },
            None => {
                if self.sent_ack_sn < self.recv_sn {
                    packet.ack = Some(self.recv_sn);
                    self.sent_ack_sn = self.recv_sn;
                }
            },
        }
    }

    pub fn add_push_json(&mut self, body_json: &str) -> Result<i64> {
        self.add_qos1_json(PacketType::Push1, body_json, None)
    }

    pub fn add_push_typed<B>(&mut self, body: &B) -> Result<i64>
    where 
        B: serde::Serialize,
    {
        self.add_qos1_typed(PacketType::Push1, body, None)
    }

    pub fn add_user_ready(&mut self, user_id: &str) -> Result<i64> {
        self.add_push_typed(&ServerPushRef::user_ready(IdRef {
            id: user_id
        }))
    }


    pub fn add_response_typed<B>(&mut self, body: &B, msg_id: Option<i64>) -> Result<i64>
    where 
        B: serde::Serialize,
    {
        self.add_qos1_typed(PacketType::Response, body, msg_id)
    }

    pub fn add_response_fail(&mut self, msg_id: Option<i64>, status: &Status) -> Result<i64> {
        let response = ServerResponseRef {
            status: Some(status),
            // msg_id,
            typ: None,
        };

        self.add_response_typed(&response, msg_id)
    }

    pub fn send_iter<'a>(&'a mut self) -> SendIter<'a> {
        SendIter {
            owner: self,
            pending: false,
        }
    }

    pub fn ack_iter<'a>(&'a mut self) -> AckIter<'a> {
        AckIter {
            owner: self,
            pending: false,
        }
    }



    pub async fn flush<F, Fut>(&mut self, mut func: F) -> Result<()> 
    where 
        F: FnMut(Packet) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        // for packet in self.send_iter() {
        //     conn.send_message(packet, "xfer_flush").await?;
        // }

        while let Some(item) = self.que.get(self.send_index) {
            debug!("send packet(qos1) [{}]", item.1);
            // conn.send_message(item.1.clone(), "xfer_flush").await?;
            func(item.1.clone()).await?;
            self.send_index += 1;
        }

        self.check_send_ack(&mut func).await
    }

    async fn check_send_ack<F, Fut>(&mut self, mut func: F) -> Result<()>
    where 
        F: FnMut(Packet) -> Fut,
        Fut: Future<Output = Result<()>>, 
    {
        if self.sent_ack_sn < self.recv_sn {
            let packet_json = PacketRef::ack_json(self.recv_sn)?;
            // conn.send_message(packet_json, "send_ack").await?;
            func(packet_json).await?;
            self.sent_ack_sn = self.recv_sn;
        }

        Ok(())
    }

    pub async fn send_push0<B, F, Fut>(&mut self, body: &B, func: F) -> Result<()>
    where 
        B: serde::Serialize,
        F: FnMut(Packet) -> Fut,
        Fut: Future<Output = Result<()>>, 
    {
        self.send_qos0(PacketType::Push0, body, func).await
    }

    async fn send_qos0<B, F, Fut>(&mut self, typ: PacketType, body: &B, mut func: F) -> Result<()>
    where 
        B: serde::Serialize,
        F: FnMut(Packet) -> Fut,
        Fut: Future<Output = Result<()>>, 
    {
        self.flush(&mut func).await?;

        let sn = self.tail_sn + 1;

        let body = serde_json::to_string(body)?;
        let mut packet = PacketRef::qos0(typ, &body);
        packet.sn = Some(sn);
        self.fill_ack(None, &mut packet);
        let json = serde_json::to_string(&packet)?;

        self.tail_sn = sn;

        // conn.send_message(json, origin).await?;
        func(json).await?;

        Ok(())
    }

    // pub async fn flush<C>(&mut self, conn: &mut C) -> Result<()> 
    // where 
    //     C: ServerConnLike,
    // {
    //     // for packet in self.send_iter() {
    //     //     conn.send_message(packet, "xfer_flush").await?;
    //     // }

    //     while let Some(item) = self.que.get(self.send_index) {
    //         debug!("send packet(qos1) [{}]", item.1);
    //         conn.send_message(item.1.clone(), "xfer_flush").await?;
    //         self.send_index += 1;
    //     }

    //     self.check_send_ack(conn).await
    // }

    // async fn check_send_ack<C>(&mut self, conn: &mut C) -> Result<()>
    // where 
    //     C: ServerConnLike, 
    // {
    //     if self.sent_ack_sn < self.recv_sn {
    //         let packet_json = PacketRef::ack_json(self.recv_sn)?;
    //         conn.send_message(packet_json, "send_ack").await?;
    //         self.sent_ack_sn = self.recv_sn;
    //     }

    //     Ok(())
    // }

    // pub async fn send_push0<C, B>(&mut self, conn: &mut C, origin: &str, body: &B) -> Result<()>
    // where 
    //     C: ServerConnLike, 
    //     B: serde::Serialize,
    // {
    //     self.send_qos0(conn, origin, PacketType::Push0, body).await
    // }

    // async fn send_qos0<C, B>(&mut self, conn: &mut C, origin: &str, typ: PacketType, body: &B) -> Result<()>
    // where 
    //     C: ServerConnLike, 
    //     B: serde::Serialize,
    // {
    //     self.flush(conn).await?;

    //     let sn = self.tail_sn + 1;

    //     let body = serde_json::to_string(body)?;
    //     let mut packet = PacketRef::qos0(typ, &body);
    //     packet.sn = Some(sn);
    //     self.fill_ack(None, &mut packet);
    //     let json = serde_json::to_string(&packet)?;

    //     self.tail_sn = sn;

    //     conn.send_message(json, origin).await?;
    //     Ok(())
    // }

    // fn send_iter(&mut self) -> SendIter<'_> {
    //     SendIter {
    //         index: (self.sent_sn - self.recv_ack_sn) as usize,
    //         owner: self, 
    //     }
    // }

}


pub struct SendIter<'a> {
    owner: &'a mut Xfer,
    pending: bool,
}

impl<'a> Iterator for SendIter<'a> {
    type Item = Packet;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pending {
            self.owner.send_index += 1;
        }

        match self.owner.que.get(self.owner.send_index) {
            Some(v) => {
                self.pending = true;
                Some(v.1.clone())
            },
            None => {
                self.pending = false;
                None
            },
        }
    }
}

pub struct AckIter<'a> {
    owner: &'a mut Xfer,
    pending: bool,
}

impl<'a> Iterator for AckIter<'a> {
    type Item = Result<Packet>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pending {
            self.owner.sent_ack_sn = self.owner.recv_sn;
            self.pending = false;
            return None
        }

        if self.owner.sent_ack_sn < self.owner.recv_sn {
            let item = PacketRef::ack_json(self.owner.recv_sn);
            self.pending = true;
            return Some(item)
        }

        self.pending = false;
        None
    }
}



