use std::collections::HashMap;

use crate::text::id::{IdReader, IdWriter};
use crate::text::num::{S3NumsReader, S3NumsWriter, S3RevNumsWriter};
use crate::text::price::{FracVariation, PricePostfixInfo, S3FloatPriceReader, S3FloatPriceWriter, S3IntPriceReader, S3IntPriceWriter};
use crate::text::s3::{RngMinimal, S3Reader, S3WriterInfo, S3WriterRand};
use crate::text::str_reader::StrReadWraper;
use crate::text::str_writer::WriteExt;
use crate::text::time::{S3TimeRW, TimeFormat};

use crate::text::s3::S3WriterRandWrap as WrapR;

pub type S3DynReader<R> = Box<dyn S3Reader<StrReadWraper<R>, Error = std::io::Error>>;
pub type S3DynWriter<W, Rng> = Box<dyn S3WriterRand<W, Rng, Error = std::io::Error>>;

pub enum S3TypeWriter<W, Rng> {
    Time(WrapR<S3TimeRW>),
    IntPrice(S3IntPriceWriter),
    FloatPrice(S3FloatPriceWriter),
    Id(IdWriter),
    IntNumRev(WrapR<S3RevNumsWriter>),
    IntNum(WrapR<S3NumsWriter>),
    Dyn(S3DynWriter<W, Rng>),
}

macro_rules! sub_call_impl_w {
    ($self:ident [$($var:ident),+] => $x:ident $call_expr:expr ) => {
        match $self {
            $(
                S3TypeWriter::$var($x) => $call_expr
            ),+
        }
    };

    ($self:ident $fn_name:ident ($($arg_name:ident),*) ) => {
        sub_call_impl_w!($self [Time, IntPrice, FloatPrice, Id, IntNumRev, IntNum, Dyn] => x x.$fn_name($($arg_name),*) )
    };

    ($self:ident $fn_name:ident) => {
        sub_call_impl_w!($self [Time, IntPrice, FloatPrice, Id, IntNumRev, IntNum, Dyn] $fn_name)
    };
}

impl<W: WriteExt, Rng: RngMinimal> S3WriterInfo for S3TypeWriter<W, Rng> {
    fn bits_once(&self) -> u8 {
        sub_call_impl_w!(self bits_once())
    }

    fn s3_once(&self) -> u64 {
        sub_call_impl_w!(self s3_once())
    }
}

impl<W: WriteExt, Rng: RngMinimal> S3WriterRand<W, Rng> for S3TypeWriter<W, Rng> {
    type Error = std::io::Error;

    fn write(&mut self, x: u64, w: &mut W, rng: &mut Rng) -> Result<(), Self::Error> {
        sub_call_impl_w!(self write(x, w, rng))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub enum S3TypeReader<R> {
    Time(S3TimeRW),
    IntPrice(S3IntPriceReader),
    FloatPrice(S3FloatPriceReader),
    Id(IdReader),
    IntNum(S3NumsReader),
    Dyn(S3DynReader<R>),
}

macro_rules! sub_call_impl_r {
    ($self:ident [$($var:ident),+] => $x:ident $call_expr:expr ) => {
        match $self {
            $(
                S3TypeReader::$var($x) => $call_expr
            ),+
        }
    };

    ($self:ident $fn_name:ident ($($arg_name:ident),*) ) => {
        sub_call_impl_r!($self [Time, IntPrice, FloatPrice, Id, IntNum, Dyn] => x x.$fn_name($($arg_name),*) )
    };

    ($self:ident $fn_name:ident) => {
        sub_call_impl_r!($self [Time, IntPrice, FloatPrice, Id, IntNum, Dyn] $fn_name)
    };
}

impl<R: std::io::Read> S3WriterInfo for S3TypeReader<R> {
    fn bits_once(&self) -> u8 {
        sub_call_impl_r!(self bits_once())
    }

    fn s3_once(&self) -> u64 {
        sub_call_impl_r!(self s3_once())
    }
}

impl<R: std::io::Read> S3Reader<StrReadWraper<R>> for S3TypeReader<R> {
    type Error = std::io::Error;

    fn read(&mut self, r: &mut StrReadWraper<R>) -> Result<u64, Self::Error> {
        sub_call_impl_r!(self read(r))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait S3CtorRW<R, W, Rng> {
    fn ctor_reader(&self, ctor_args: &[u8]) -> S3DynReader<R>;
    fn ctor_writer(&self, ctor_args: &[u8]) -> S3DynWriter<W, Rng>;
}

pub struct S3CtorsRW<R, W, Rng> {
    map: HashMap<u32, Box<dyn S3CtorRW<R, W, Rng>>>,
}
impl<R, W, Rng> S3CtorsRW<R, W, Rng> {
    pub fn new() -> Self {
        Self { map: HashMap::with_capacity(16) }
    }

    pub fn is_unused_id(&self, id: u32) -> bool {
        self.map.contains_key(&id)
    }
    
    /// # Panics 
    /// * if `!self.is_unused_id(id)`
    pub fn add_ctor(&mut self, id: u32, ctor: Box<dyn S3CtorRW<R, W, Rng>>) {
        if self.map.insert(id, ctor).is_some() {
            panic!("S3CtorsRW: id ({id}) is already used!")
        }
    }

    /// # Panics 
    /// * if `self.is_unused_id(id)`
    pub fn ctor_reader(&self, id: u32, ctor_args: &[u8]) -> S3DynReader<R> {
        match self.map.get(&id) {
            Some(ctor) => ctor.ctor_reader(ctor_args),
            None => panic!("S3CtorsRW(ctor_reader): unknown id ({id})!"),
        }
    }
    
    /// # Panics 
    /// * if `self.is_unused_id(id)`
    pub fn ctor_writer(&self, id: u32, ctor_args: &[u8]) -> S3DynWriter<W, Rng> {
        match self.map.get(&id) {
            Some(ctor) => ctor.ctor_writer(ctor_args),
            None => panic!("S3CtorsRW(ctor_reader): unknown id ({id})!"),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub enum S3Type {
    Time(TimeFormat),
    IntPrice{ int_len: u8, prefix_range: u8, postfix: PricePostfixInfo },
    FloatPrice{ int_len: u8, prefix_range: u8, postfix: PricePostfixInfo, frac: FracVariation },
    Id { prefix_start_from: u32, hide_len: u8, postfix_len: u8 },
    IntNumRev { num_len: u8, zeroed: bool },
    IntNum { num_len: u8, zeroed: bool },
    Dyn { dyn_id: u32, args: Vec<u8> },
}

impl S3Type {
        pub fn new_time(fmt: TimeFormat) -> Self {
            Self::Time(fmt)
        }

        pub fn new_price_int(int_len: u8, prefix_range: u8, postfix: PricePostfixInfo) -> Self {
            Self::IntPrice { int_len, prefix_range, postfix }
        }

        pub fn new_price_float(int_len: u8, prefix_range: u8, postfix: PricePostfixInfo, frac: FracVariation) -> Self {
            Self::FloatPrice { int_len, prefix_range, postfix, frac }
        }

        pub fn new_id(prefix_start_from: u32, hide_len: u8, postfix_len: u8) -> Self {
            Self::Id { prefix_start_from, hide_len, postfix_len }
        }

        pub fn new_int_num(num_len: u8, zeroed: bool) -> Self {
            Self::IntNum { num_len, zeroed }
        }

        pub fn new_int_num_rev(num_len: u8, zeroed: bool) -> Self {
            Self::IntNumRev { num_len, zeroed }
        }

        pub fn new_dyn(dyn_id: u32, args: Vec<u8>) -> Self {
            Self::Dyn { dyn_id, args }
        }

        pub fn is_dyn(&self) -> bool {
            matches!(self, Self::Dyn { .. })
        }

        pub fn to_dyn_writer<R, W, Rng>(&self, ctors: &S3CtorsRW<R, W, Rng>) -> S3TypeWriter<W, Rng> {
            match self {
                Self::Dyn { dyn_id, args } => {
                    S3TypeWriter::Dyn(ctors.ctor_writer(*dyn_id, args.as_slice()))
                }
                _ => self.to_writer(),
            }
        }

        pub fn to_dyn_reader<R, W, Rng>(&self, ctors: &S3CtorsRW<R, W, Rng>) -> S3TypeReader<R> {
            match self {
                Self::Dyn { dyn_id, args } => {
                    S3TypeReader::Dyn(ctors.ctor_reader(*dyn_id, args.as_slice()))
                }
                _ => self.to_reader(),
            }
        }

        /// # Panics
        /// * if `self.is_dyn()`
        pub fn to_writer<W, Rng>(&self) -> S3TypeWriter<W, Rng> {
        match *self {
            S3Type::Time(fmt) => S3TypeWriter::Time(WrapR(S3TimeRW::new(fmt))),

            S3Type::IntPrice { int_len, prefix_range, postfix } 
            => S3TypeWriter::IntPrice(S3IntPriceWriter::new(int_len, prefix_range, postfix)),
            
            S3Type::FloatPrice { int_len, prefix_range, postfix, frac }
            => S3TypeWriter::FloatPrice(S3FloatPriceWriter::new(
                S3IntPriceWriter::new(int_len, prefix_range, postfix),
                frac,
            )),
            
            S3Type::Id { prefix_start_from, hide_len, postfix_len } 
            => S3TypeWriter::Id(IdWriter::new(prefix_start_from as u64, hide_len, postfix_len)),
            
            S3Type::IntNumRev { num_len, zeroed } => S3TypeWriter::IntNumRev(WrapR(S3RevNumsWriter::new(num_len, zeroed))),
            S3Type::IntNum { num_len, zeroed } => S3TypeWriter::IntNum(WrapR(S3NumsWriter::new(num_len, zeroed))),
            S3Type::Dyn { .. } => panic!("S3Type(to_writer): self is dyn"),
        }
    }

    /// # Panics
    /// * if `self.is_dyn()`
    pub fn to_reader<R>(&self) -> S3TypeReader<R> {
        match *self {
            S3Type::Time(fmt) => S3TypeReader::Time(S3TimeRW::new(fmt)),
            S3Type::IntPrice { int_len, postfix, .. } => S3TypeReader::IntPrice(S3IntPriceReader::new(int_len, postfix)),
            
            S3Type::FloatPrice { int_len, postfix, frac, .. }
            => S3TypeReader::FloatPrice(S3FloatPriceReader::new(
                S3IntPriceReader::new(int_len, postfix),
                frac,
            )),
            S3Type::Id { hide_len, postfix_len, .. } => S3TypeReader::Id(IdReader::new(hide_len, postfix_len)),
            S3Type::IntNumRev { num_len, zeroed } => S3TypeReader::IntNum(S3NumsReader::new(num_len, true, zeroed)),
            S3Type::IntNum { num_len, zeroed } => S3TypeReader::IntNum(S3NumsReader::new(num_len, false, zeroed)),
            S3Type::Dyn { .. } => panic!("S3Type(to_reader): self is dyn"),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct S3Signature {
    s3_types: Vec<S3Type>,
}
impl S3Signature {
    pub fn new() -> Self {
        Self::with_capacity(10)
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self { s3_types: Vec::with_capacity(capacity) }
    }

    pub fn add_s3_type(&mut self, s3_type: S3Type) {
        self.s3_types.push(s3_type);
    }

    pub fn iter_readers<'a, R, W, Rng>(&'a self, ctors: &'a S3CtorsRW<R, W, Rng>) -> impl Iterator<Item = S3TypeReader<R>> + 'a {
        self.s3_types.iter().map(|x|x.to_dyn_reader(ctors))
    }
    
    pub fn iter_writers<'a, R, W, Rng>(&'a self, ctors: &'a S3CtorsRW<R, W, Rng>) -> impl Iterator<Item = S3TypeWriter<W, Rng>> + 'a {
        self.s3_types.iter().map(|x|x.to_dyn_writer(ctors))
    }
}