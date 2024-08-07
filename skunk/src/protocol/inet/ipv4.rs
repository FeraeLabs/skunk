use std::{
    convert::Infallible,
    fmt::Debug,
    net::Ipv4Addr,
};

use bitflags::bitflags;
use byst::{
    endianness::NetworkEndian,
    io::{
        read,
        Limit,
        Read,
        Reader,
        ReaderExt,
        Write,
        Writer,
        WriterExt,
    },
    Bytes,
};

use super::udp;
use crate::util::network_enum;

#[derive(Clone, Debug)]
pub struct Header {
    pub version: u8,
    pub internet_header_length: u8,
    pub differentiated_service_code_point: u8,
    pub explicit_congestion_notification: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags: Flags,
    pub fragment_offset: u16,
    pub time_to_live: u8,
    pub protocol: Protocol,
    pub header_checksum: u16,
    pub source_address: Ipv4Addr,
    pub destination_address: Ipv4Addr,
    //pub options: Options,
}

impl<R: Reader> Read<R, ()> for Header {
    type Error = InvalidHeader<R::Error>;

    fn read(reader: &mut R, _params: ()) -> Result<Self, Self::Error> {
        let version_ihl = read!(reader => u8)?;
        let version = version_ihl >> 4;
        if version != 4 {
            return Err(InvalidHeader::InvalidVersion { value: version });
        }

        let internet_header_length = version_ihl & 0xf;
        if internet_header_length != 5 {
            // todo: support options
            return Err(InvalidHeader::InvalidInternetHeaderLength {
                value: internet_header_length,
            });
        }

        let dscp_ecn = read!(reader => u8)?;
        let differentiated_service_code_point = dscp_ecn >> 2;
        let explicit_congestion_notification = dscp_ecn & 3;

        let total_length = read!(reader; NetworkEndian)?;

        let identification = read!(reader; NetworkEndian)?;

        let flags_fragment_offset = read!(reader => u16; NetworkEndian)?;
        let flags = Flags::from_bits_retain((flags_fragment_offset >> 13) as u8);
        let fragment_offset = flags_fragment_offset & 0x1fff;

        let time_to_live = read!(reader)?;

        let protocol = read!(reader)?;

        let header_checksum = read!(reader; NetworkEndian)?;

        let source_address = read!(reader)?;

        let destination_address = read!(reader)?;

        Ok(Self {
            version,
            internet_header_length,
            differentiated_service_code_point,
            explicit_congestion_notification,
            total_length,
            identification,
            flags,
            fragment_offset,
            time_to_live,
            protocol,
            header_checksum,
            source_address,
            destination_address,
            //options,
        })
    }
}

impl<W: Writer> Write<W, ()> for Header {
    type Error = W::Error;

    fn write(&self, _writer: &mut W, _context: ()) -> Result<(), Self::Error> {
        //writer.write(&(self.version << 4 | self.internet_header_length))?;
        Ok(())
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Read, Write)]
    pub struct Flags: u8 {
        const RESERVED = 0b100;
        const DONT_FRAGMENT = 0b010;
        const MORE_FRAGMENTS = 0b001;
    }
}

#[derive(Clone, Debug)]
pub struct Packet<P = Bytes> {
    pub header: Header,
    pub payload: P,
}

impl<R: Reader, P, E> Read<R, ()> for Packet<P>
where
    P: for<'r> Read<Limit<&'r mut R>, Protocol, Error = E>,
{
    type Error = InvalidPacket<R::Error, E>;

    fn read(reader: &mut R, _params: ()) -> Result<Self, Self::Error> {
        let header: Header = reader.read()?;

        let payload_length =
            usize::from(header.total_length) - usize::from(header.internet_header_length);
        let mut limit = reader.limit(payload_length);
        let payload = limit
            .read_with(header.protocol)
            .map_err(InvalidPacket::Payload)?;
        let _ = limit.skip_remaining();

        Ok(Self { header, payload })
    }
}

impl<W: Writer, P> Write<W, ()> for Packet<P>
where
    P: Write<W, (), Error = W::Error>,
{
    type Error = W::Error;

    fn write(&self, writer: &mut W, _context: ()) -> Result<(), Self::Error> {
        writer.write(&self.header)?;
        writer.write(&self.payload)?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid IPv4 packet")]
pub enum InvalidPacket<R, P = Infallible> {
    Header(#[from] InvalidHeader<R>),
    Payload(#[source] P),
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid IPv4 header")]
pub enum InvalidHeader<R> {
    Read(#[from] R),

    #[error("Invalid IP version: {value}")]
    InvalidVersion {
        value: u8,
    },

    #[error("Invalid internet header length: {value}")]
    InvalidInternetHeaderLength {
        value: u8,
    },
}

#[derive(Clone, Debug)]
pub enum AnyPayload<P = Bytes> {
    Udp(udp::Packet<P>),
    Unknown(P),
}

impl<R: Reader, P, E> Read<R, Protocol> for AnyPayload<P>
where
    P: Read<R, (), Error = E>,
{
    type Error = AnyPayloadError<E>;

    fn read(_reader: &mut R, _protocol: Protocol) -> Result<Self, Self::Error> {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("IPv4 payload error")]
pub enum AnyPayloadError<R> {
    Udp(#[from] udp::InvalidPacket<R>),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Read)]
pub struct Protocol(pub u8);

network_enum! {
    for Protocol: Debug;

    /// Internet Control Message Protocol
    ICMP => 0x01;

    /// Transmission Control Protocol
    TCP => 0x06;

    /// User Datagram Protocol
    UDP => 0x11;
}
