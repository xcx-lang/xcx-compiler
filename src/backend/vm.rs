use std::sync::Arc;
use parking_lot::{RwLock, RwLockWriteGuard, Mutex};
use std::sync::atomic::{AtomicBool, Ordering, AtomicPtr};
pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);
use argon2::password_hash::{PasswordHasher, PasswordVerifier, PasswordHash, SaltString};
use std::io::Write;
use std::ptr;
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Push, Pop, Len, Count, Size, IsEmpty, Clear, Contains, Get, Insert, Update, Delete, Find, Join, Show, Sort, Reverse,
    Add, Remove, Has, Length, Upper, Lower, Trim, IndexOf, LastIndexOf, Replace, Slice, Split, StartsWith, EndsWith,
    ToInt, ToFloat, Set, Keys, Values, Where, Year, Month, Day, Hour, Minute, Second, Format, Exists, Append, Inject, ToStr,
    Next, Run, IsDone, Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCode {
    Move { dst: u8, src: u8 },
    LoadConst { dst: u8, idx: u32 },
    
    Add { dst: u8, src1: u8, src2: u8 },
    Sub { dst: u8, src1: u8, src2: u8 },
    Mul { dst: u8, src1: u8, src2: u8 },
    Div { dst: u8, src1: u8, src2: u8 },
    Mod { dst: u8, src1: u8, src2: u8 },
    Pow { dst: u8, src1: u8, src2: u8 },
    
    Equal { dst: u8, src1: u8, src2: u8 },
    NotEqual { dst: u8, src1: u8, src2: u8 },
    Greater { dst: u8, src1: u8, src2: u8 },
    Less { dst: u8, src1: u8, src2: u8 },
    GreaterEqual { dst: u8, src1: u8, src2: u8 },
    LessEqual { dst: u8, src1: u8, src2: u8 },
    
    And { dst: u8, src1: u8, src2: u8 },
    Or { dst: u8, src1: u8, src2: u8 },
    Not { dst: u8, src: u8 },
    Has { dst: u8, src1: u8, src2: u8 },
    
    GetVar { dst: u8, idx: u32 },
    SetVar { idx: u32, src: u8 },

    Jump { target: u32 },
    JumpIfFalse { src: u8, target: u32 },
    JumpIfTrue { src: u8, target: u32 },
    
    Print { src: u8 },
    Input { dst: u8 },
    HaltAlert { src: u8 }, 
    HaltError { src: u8 }, 
    HaltFatal { src: u8 },
    TerminalExit,
    TerminalRun { dst: u8, cmd_src: u8 },

    TerminalClear,

    
    Call { dst: u8, func_idx: u32, base: u8, arg_count: u8 },
    Return { src: u8 },
    ReturnVoid,
    Halt,

    // Collections
    ArrayInit { dst: u8, base: u8, count: u32 },
    SetInit { dst: u8, base: u8, count: u32 },
    MapInit { dst: u8, base: u8, count: u32 },
    TableInit { dst: u8, skeleton_idx: u32, base: u8, row_count: u32 },
    
    MethodCall { dst: u8, kind: MethodKind, base: u8, arg_count: u8 },
    MethodCallCustom { dst: u8, method_name_idx: u32, base: u8, arg_count: u8 },

    SetUnion { dst: u8, src1: u8, src2: u8 },
    SetIntersection { dst: u8, src1: u8, src2: u8 },
    SetDifference { dst: u8, src1: u8, src2: u8 },
    SetSymDifference { dst: u8, src1: u8, src2: u8 },
    RandomChoice { dst: u8, src: u8 },
    IntConcat { dst: u8, src1: u8, src2: u8 },
    SetRange { dst: u8, start: u8, end: u8, step: u8, has_step: u8 },

    // Store operations
    StoreWrite { base: u8 }, 
    StoreRead { dst: u8, base: u8 },
    StoreAppend { base: u8 },
    StoreExists { dst: u8, base: u8 },
    StoreDelete { base: u8 },

    // JSON/Date
    JsonParse { dst: u8, src: u8 },
    DateNow { dst: u8 },
    JsonBind { idx: u32, json_src: u8, path_src: u8 },
    JsonBindLocal { dst: u8, json_src: u8, path_src: u8 },
    JsonInject { table_idx: u32, json_src: u8, mapping_src: u8 },
    JsonInjectLocal { table_reg: u8, json_src: u8, mapping_src: u8 },

    // Fibers/Concurrency
    FiberCreate { dst: u8, func_idx: u32, base: u8, arg_count: u8 },
    Yield { src: u8 },
    YieldVoid,
    Wait { src: u8 },

    // HTTP
    HttpCall { dst: u8, method_idx: u32, url_src: u8, body_src: u8 },
    HttpRequest { dst: u8, arg_src: u8 },
    HttpRespond { status_src: u8, body_src: u8, headers_src: u8 },
    HttpServe { func_idx: u32, port_src: u8, host_src: u8, workers_src: u8, routes_src: u8 },

    // Misc and Casts
    EnvGet { dst: u8, src: u8 },
    EnvArgs { dst: u8 },
    CryptoHash { dst: u8, pass_src: u8, alg_src: u8 },
    CryptoVerify { dst: u8, pass_src: u8, hash_src: u8, alg_src: u8 },
    CryptoToken { dst: u8, len_src: u8 },
    
    CastInt { dst: u8, src: u8 },
    CastFloat { dst: u8, src: u8 },
    CastString { dst: u8, src: u8 },
    CastBool { dst: u8, src: u8 },

    // Optimizations
    IncLocal { reg: u8 },
    LoopNext { reg: u8, limit_reg: u8, target: u32 },
    IncLocalLoopNext { inc_reg: u8, reg: u8, limit_reg: u8, target: u32 },
    IncVar { idx: u32 },
    IncVarLoopNext { g_idx: u32, reg: u8, limit_reg: u8, target: u32 },
}

#[derive(Clone, Debug)]
pub enum TraceOp {
    LoadConst { dst: u8, val: Value },
    Move      { dst: u8, src: u8   },

    // Integer arithmetic (all operands must be NaN-boxed ints)
    AddInt { dst: u8, src1: u8, src2: u8 },
    SubInt { dst: u8, src1: u8, src2: u8 },
    MulInt { dst: u8, src1: u8, src2: u8 },
    DivInt { dst: u8, src1: u8, src2: u8, fail_ip: usize },
    ModInt { dst: u8, src1: u8, src2: u8, fail_ip: usize },

    // Float arithmetic
    AddFloat { dst: u8, src1: u8, src2: u8 },
    SubFloat { dst: u8, src1: u8, src2: u8 },
    MulFloat { dst: u8, src1: u8, src2: u8 },
    DivFloat { dst: u8, src1: u8, src2: u8, fail_ip: usize },
    ModFloat { dst: u8, src1: u8, src2: u8, fail_ip: usize },

    IncLocal { reg: u8 },
    IncVar   { g_idx: u32 },

    GetVar { dst: u8, idx: u32 },
    SetVar { idx: u32, src: u8 },

    GuardInt   { reg: u8, ip: usize },
    GuardFloat { reg: u8, ip: usize },

    CmpInt   { dst: u8, src1: u8, src2: u8, cc: u8 },
    CmpFloat { dst: u8, src1: u8, src2: u8, cc: u8 },
    
    CastIntToFloat { dst: u8, src: u8 },

    GuardTrue  { reg: u8, fail_ip: usize },
    GuardFalse { reg: u8, fail_ip: usize },

    // Loop control
    LoopNextInt      { reg: u8, limit_reg: u8, target: u32, exit_ip: usize },
    IncVarLoopNext   { g_idx: u32, reg: u8, limit_reg: u8, target: u32, exit_ip: usize },
    IncLocalLoopNext { inc_reg: u8, reg: u8, limit_reg: u8, target: u32, exit_ip: usize },

    // Logic ops
    And { dst: u8, src1: u8, src2: u8 },
    Or  { dst: u8, src1: u8, src2: u8 },
    Not { dst: u8, src: u8 },

    Jump { target_ip: usize },
}

#[derive(Debug)]
pub struct Trace {
    pub ops: Vec<TraceOp>,
    pub start_ip: usize,
    pub native_ptr: std::sync::atomic::AtomicPtr<u8>,
}

pub struct RowRef {
    pub table: Arc<RwLock<TableData>>,
    pub row_idx: u32,
}

#[derive(Debug, Clone)]
pub struct SetData {
    pub elements: std::collections::BTreeSet<Value>,
    pub cache: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Value(pub u64);

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        if self.is_float() && other.is_float() {
            return self.as_f64() == other.as_f64();
        }
        if self.is_string() && other.is_string() {
            return *self.as_string() == *other.as_string();
        }
        self.0 == other.0
    }
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let r1 = self.variant_rank();
        let r2 = other.variant_rank();
        if r1 != r2 { return r1.cmp(&r2); }
        
        if self.is_int() && other.is_int() { return self.as_i64().cmp(&other.as_i64()); }
        if self.is_float() && other.is_float() {
            return self.as_f64().partial_cmp(&other.as_f64()).unwrap_or(std::cmp::Ordering::Equal);
        }
        if self.is_bool() && other.is_bool() { return self.as_bool().cmp(&other.as_bool()); }
        
        if self.is_ptr() && other.is_ptr() {
            let tag = self.0 & 0x000F_0000_0000_0000;
            if tag == TAG_STR { return self.as_string().cmp(&other.as_string()); }
            if tag == TAG_DATE { return self.as_date().cmp(&other.as_date()); }
        }
        
        self.0.cmp(&other.0)
    }
}

pub const QNAN_BASE: u64 = 0x7FF0_0000_0000_0000;
pub const TAG_INT:   u64 = 0x0001_0000_0000_0000;
pub const TAG_BOOL:  u64 = 0x0002_0000_0000_0000;
pub const TAG_DATE:  u64 = 0x0003_0000_0000_0000;
pub const TAG_STR:   u64 = 0x0004_0000_0000_0000;
pub const TAG_ARR:   u64 = 0x0005_0000_0000_0000;
pub const TAG_SET:   u64 = 0x0006_0000_0000_0000;
pub const TAG_MAP:   u64 = 0x0007_0000_0000_0000;
pub const TAG_TBL:   u64 = 0x0008_0000_0000_0000;
pub const TAG_FUNC:  u64 = 0x0009_0000_0000_0000;
pub const TAG_ROW:   u64 = 0x000A_0000_0000_0000;
pub const TAG_JSON:  u64 = 0x000B_0000_0000_0000;
pub const TAG_FIB:   u64 = 0x000C_0000_0000_0000;

impl Value {
    #[inline] pub fn from_f64(f: f64) -> Self {
        let b = f.to_bits();
        if (b & QNAN_BASE) == QNAN_BASE { Self(QNAN_BASE | 0x1) }
        else { Self(b) }
    }
    #[inline] pub fn from_i64(i: i64) -> Self { Self(QNAN_BASE | TAG_INT | ((i as u64) & 0x0000_FFFF_FFFF_FFFF)) }
    #[inline] pub fn from_bool(b: bool) -> Self { Self(QNAN_BASE | TAG_BOOL | (if b { 1 } else { 0 })) }
    
    #[inline] pub fn pack_ptr<T>(ptr: *const T, tag: u64) -> Self {
        Self(QNAN_BASE | tag | (ptr as u64 & 0x0000_FFFF_FFFF_FFFF))
    }
    #[inline] pub fn unpack_ptr<T>(&self) -> *const T {
        (self.0 & 0x0000_FFFF_FFFF_FFFF) as *const T
    }

    #[inline] pub fn is_float(&self) -> bool { (self.0 & 0x7FF0_0000_0000_0000) != 0x7FF0_0000_0000_0000 }
    #[inline] pub fn is_int(&self)   -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) }
    #[inline] pub fn is_bool(&self)  -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_BOOL) }
    #[inline] pub fn is_ptr(&self)   -> bool { 
        (self.0 & 0xFFF0_0000_0000_0000) == QNAN_BASE && (self.0 & 0x000F_0000_0000_0000) >= TAG_STR
    }
    #[inline] pub fn is_string(&self) -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_STR) }
    #[inline] pub fn is_date(&self)   -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_DATE) }
    #[inline] pub fn is_func(&self)   -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_FUNC) }
    #[inline] pub fn is_array(&self)  -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_ARR) }
    #[inline] pub fn is_map(&self)    -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_MAP) }

    #[inline] pub fn as_f64(&self) -> f64 { f64::from_bits(self.0) }
    #[inline] pub fn as_i64(&self) -> i64 { 
        let val = self.0 & 0x0000_FFFF_FFFF_FFFF;
        if val & 0x0000_8000_0000_0000 != 0 { (val | 0xFFFF_0000_0000_0000) as i64 }
        else { val as i64 }
    }
    #[inline] pub fn as_bool(&self) -> bool { (self.0 & 1) != 0 }

    #[inline]
    pub unsafe fn inc_ref(&self) {
        if !self.is_ptr() { return; }
        let tag = self.0 & 0x000F_0000_0000_0000;
        let p = self.unpack_ptr::<()>();
        match tag {
            TAG_STR  => { unsafe { Arc::increment_strong_count(p as *const String); } }
            TAG_ARR  => { unsafe { Arc::increment_strong_count(p as *const RwLock<Vec<Value>>); } }
            TAG_SET  => { unsafe { Arc::increment_strong_count(p as *const RwLock<SetData>); } }
            TAG_MAP  => { unsafe { Arc::increment_strong_count(p as *const RwLock<Vec<(Value, Value)>>); } }
            TAG_TBL  => { unsafe { Arc::increment_strong_count(p as *const RwLock<TableData>); } }
            TAG_JSON => { unsafe { Arc::increment_strong_count(p as *const RwLock<serde_json::Value>); } }
            TAG_FIB  => { unsafe { Arc::increment_strong_count(p as *const RwLock<FiberState>); } }
            TAG_ROW  => { unsafe { Arc::increment_strong_count(p as *const RowRef); } }
            _ => {}
        }
    }

    #[inline]
    pub unsafe fn dec_ref(&self) {
        if !self.is_ptr() { return; }
        let tag = self.0 & 0x000F_0000_0000_0000;
        let p = self.unpack_ptr::<()>();
        match tag {
            TAG_STR  => { unsafe { Arc::decrement_strong_count(p as *const String); } }
            TAG_ARR  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<Vec<Value>>); } }
            TAG_SET  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<SetData>); } }
            TAG_MAP  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<Vec<(Value, Value)>>); } }
            TAG_TBL  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<TableData>); } }
            TAG_JSON => { unsafe { Arc::decrement_strong_count(p as *const RwLock<serde_json::Value>); } }
            TAG_FIB  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<FiberState>); } }
            TAG_ROW  => { unsafe { Arc::decrement_strong_count(p as *const RowRef); } }
            _ => {}
        }
    }

    #[inline] pub fn is_numeric(&self) -> bool { self.is_float() || self.is_int() }
    
    pub fn variant_rank(&self) -> u8 {
        if self.is_float() { 1 }
        else if self.is_int() { 0 }
        else if (self.0 & 0xFFF0_0000_0000_0000) == QNAN_BASE {
            let tag = self.0 & 0x000F_0000_0000_0000;
            match tag {
                TAG_BOOL => 2,
                TAG_STR  => 3,
                TAG_ARR  => 4,
                TAG_SET  => 5,
                TAG_MAP  => 6,
                TAG_DATE => 7,
                TAG_TBL  => 8,
                TAG_FUNC => 9,
                TAG_ROW  => 10,
                TAG_JSON => 11,
                TAG_FIB  => 12,
                _ => 255,
            }
        } else { 255 }
    }

    #[inline] pub fn from_string(s: Arc<String>) -> Self { Self::pack_ptr(Arc::into_raw(s), TAG_STR) }
    #[inline] pub fn from_array(a: Arc<RwLock<Vec<Value>>>) -> Self { Self::pack_ptr(Arc::into_raw(a), TAG_ARR) }
    #[inline] pub fn from_set(s: Arc<RwLock<SetData>>) -> Self { Self::pack_ptr(Arc::into_raw(s), TAG_SET) }
    #[inline] pub fn from_map(m: Arc<RwLock<Vec<(Value, Value)>>>) -> Self { Self::pack_ptr(Arc::into_raw(m), TAG_MAP) }
    #[inline] pub fn from_table(t: Arc<RwLock<TableData>>) -> Self { Self::pack_ptr(Arc::into_raw(t), TAG_TBL) }
    #[inline] pub fn from_json(j: Arc<RwLock<serde_json::Value>>) -> Self { Self::pack_ptr(Arc::into_raw(j), TAG_JSON) }
    #[inline] pub fn from_fiber(f: Arc<RwLock<FiberState>>) -> Self { Self::pack_ptr(Arc::into_raw(f), TAG_FIB) }
    #[inline] pub fn from_date(ts: i64) -> Self { Self(QNAN_BASE | TAG_DATE | (ts as u64 & 0x0000_FFFF_FFFF_FFFF)) }
    #[inline] pub fn from_function(id: u32) -> Self { Self(QNAN_BASE | TAG_FUNC | (id as u64)) }
    #[inline] pub fn from_row(r: Arc<RowRef>) -> Self { Self::pack_ptr(Arc::into_raw(r), TAG_ROW) }

    pub fn as_string(&self) -> Arc<String> { unsafe { let p = self.unpack_ptr::<String>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_array(&self) -> Arc<RwLock<Vec<Value>>> { unsafe { let p = self.unpack_ptr::<RwLock<Vec<Value>>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_set(&self) -> Arc<RwLock<SetData>> { unsafe { let p = self.unpack_ptr::<RwLock<SetData>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_map(&self) -> Arc<RwLock<Vec<(Value, Value)>>> { unsafe { let p = self.unpack_ptr::<RwLock<Vec<(Value, Value)>>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_table(&self) -> Arc<RwLock<TableData>> { unsafe { let p = self.unpack_ptr::<RwLock<TableData>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_json(&self) -> Arc<RwLock<serde_json::Value>> { unsafe { let p = self.unpack_ptr::<RwLock<serde_json::Value>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_fiber(&self) -> Arc<RwLock<FiberState>> { unsafe { let p = self.unpack_ptr::<RwLock<FiberState>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_row(&self) -> Arc<RowRef> { unsafe { let p = self.unpack_ptr::<RowRef>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    #[inline] pub fn as_date(&self) -> i64 { (self.0 & 0x0000_FFFF_FFFF_FFFF) as i64 }
    #[inline] pub fn as_function(&self) -> u32 { (self.0 & 0x0000_FFFF_FFFF_FFFF) as u32 }

    pub fn to_string(&self) -> String {
        if self.is_float() { self.as_f64().to_string() }
        else if self.is_int() { self.as_i64().to_string() }
        else if self.is_bool() { self.as_bool().to_string() }
        else if self.is_ptr() {
            let tag = self.0 & 0x000F_0000_0000_0000;
            match tag {
                TAG_STR => { unsafe { (&*(self.unpack_ptr::<String>())).clone() } }
                TAG_DATE => { 
                    let ts = self.as_date(); 
                    let dt = chrono::DateTime::from_timestamp_millis(ts).unwrap().naive_utc();
                    dt.format("%Y-%m-%d").to_string()
                }
                TAG_JSON => {
                    let arc = self.as_json();
                    arc.read().to_string()
                }
                TAG_ARR  => { 
                    let arc = self.as_array();
                    let a = arc.read();
                    let mut s = "[".to_string();
                    for (i, val) in a.iter().enumerate() {
                        if i > 0 { s.push_str(", "); }
                        s.push_str(&val.to_string());
                    }
                    s.push(']');
                    s
                }
                TAG_SET  => {
                    let arc = self.as_set();
                    let set_data = arc.read();
                    let mut s = "{".to_string();
                    for (i, val) in set_data.elements.iter().enumerate() {
                        if i > 0 { s.push_str(", "); }
                        s.push_str(&val.to_string());
                    }
                    s.push('}');
                    s
                }
                TAG_MAP  => {
                    let arc = self.as_map();
                    let map = arc.read();
                    let mut s = "{".to_string();
                    for (i, (k, v)) in map.iter().enumerate() {
                        if i > 0 { s.push_str(", "); }
                        s.push_str(&format!("{} :: {}", k, v));
                    }
                    s.push('}');
                    s
                }
                _ => format!("Ptr({:x})", self.0),
            }
        }
        else { format!("Value({:x})", self.0) }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct VMColumn {
    pub name: String,
    pub ty: crate::parser::ast::Type,
    pub is_auto: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableData {
    pub columns: Vec<VMColumn>,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Clone)]
pub struct FiberState {
    pub func_id: usize,
    pub ip: usize,
    pub locals: Vec<Value>,
    pub is_done: bool,
    pub yielded_value: Option<Value>,
}

impl PartialEq for FiberState {
    fn eq(&self, other: &Self) -> bool { std::ptr::eq(self, other) }
}
impl Eq for FiberState {}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_float() { write!(f, "{}", self.as_f64()) }
        else if self.is_int() { write!(f, "{}", self.as_i64()) }
        else if self.is_bool() { write!(f, "{}", self.as_bool()) }
        else if self.is_ptr() {
            let tag = self.0 & 0x000F_0000_0000_0000;
            match tag {
                TAG_STR  => write!(f, "{}", self.as_string()),
                TAG_ARR  => {
                    let arc = self.as_array();
                    let arr = arc.read();
                    write!(f, "[")?;
                    for (i, val) in arr.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", val)?;
                    }
                    write!(f, "]")
                }
                TAG_SET  => {
                    let arc = self.as_set();
                    let s = arc.read();
                    write!(f, "{{")?;
                    for (i, val) in s.elements.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", val)?;
                    }
                    write!(f, "}}")
                }
                TAG_MAP  => {
                    let arc = self.as_map();
                    let m = arc.read();
                    write!(f, "{{")?;
                    for (i, (k, v)) in m.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{} :: {}", k, v)?;
                    }
                    write!(f, "}}")
                }
                TAG_DATE => {
                    let ts = self.as_date();
                    let dt = chrono::DateTime::from_timestamp_millis(ts).unwrap().naive_utc();
                    write!(f, "{}", dt.format("%Y-%m-%d"))
                }
                TAG_TBL  => {
                    let arc = self.as_table();
                    write!(f, "Table(rows: {})", arc.read().rows.len())
                }
                TAG_FUNC => write!(f, "Function({})", self.as_function()),
                TAG_ROW  => write!(f, "Row({})", self.as_row().row_idx),
                TAG_JSON => {
                    let arc = self.as_json();
                    write!(f, "Json({})", arc.read())
                }
                TAG_FIB  => {
                    let arc = self.as_fiber();
                    let fib = arc.read();
                    if fib.is_done { write!(f, "Fiber(done)") }
                    else { write!(f, "Fiber(ip={})", fib.ip) }
                }
                _ => write!(f, "Ptr({:x})", self.0),
            }
        } else {
            write!(f, "Value({:x})", self.0)
        }
    }
}

#[derive(Clone)]
pub struct FunctionChunk {
    pub bytecode: Arc<Vec<OpCode>>,
    pub spans: Arc<Vec<crate::lexer::token::Span>>,
    pub is_fiber: bool,
    pub max_locals: usize,
}

#[derive(Clone)]
pub struct SharedContext {
    pub constants: Arc<Vec<Value>>,
    pub functions: Arc<Vec<FunctionChunk>>,
}

pub struct VM {
    pub globals: Arc<RwLock<Vec<Value>>>,
    pub error_count: std::sync::atomic::AtomicUsize,
    pub traces: Arc<RwLock<std::collections::HashMap<usize, Arc<Trace>>>>,
    pub jit: parking_lot::Mutex<crate::backend::jit::JIT>,
}

#[derive(Debug, Clone)]
enum OpResult {
    Continue,
    Return(Option<Value>),
    Yield(Option<Value>),
    Halt,
}



impl VM {
    pub fn new() -> Self {
        #[cfg(all(windows, not(test)))]
        enable_ansi_support();
        Self {
            globals: Arc::new(RwLock::new(vec![Value::from_bool(false); 1024])),
            error_count: std::sync::atomic::AtomicUsize::new(0),
            traces: Arc::new(RwLock::new(std::collections::HashMap::new())),
            jit: parking_lot::Mutex::new(crate::backend::jit::JIT::new()),
        }
    }

    #[allow(dead_code)]
    pub fn get_global(&self, idx: usize) -> Option<Value> {
        self.globals.read().get(idx).cloned()
    }

    pub fn run(self: Arc<Self>, main_chunk: FunctionChunk, ctx: SharedContext) {
        let mut executor = Executor {
            vm: self.clone(),
            ctx,
            current_spans: None,
            fiber_yielded: false,
            hot_counts: vec![0; main_chunk.bytecode.len()],
            recording_trace: None,
            is_recording: false,
            trace_cache: vec![None; main_chunk.bytecode.len()],
            http_req: None,
            http_req_val: None,
        };
        // Populate initial cache
        {
            let traces = executor.vm.traces.read();
            for (ip, trace) in traces.iter() {
                if *ip < executor.trace_cache.len() {
                    executor.trace_cache[*ip] = Some(trace.clone());
                }
            }
        }
        executor.run_frame_owned(main_chunk);
    }
}

#[cfg(windows)]
fn enable_ansi_support() {
    use std::ptr;
    type DWORD = u32;
    type HANDLE = *mut std::ffi::c_void;
    const STD_OUTPUT_HANDLE: DWORD = -11i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: DWORD = 0x0004;
    unsafe extern "system" {
        fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;
        fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut DWORD) -> i32;
        fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: DWORD) -> i32;
    }
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle != ptr::null_mut() {
            let mut mode: DWORD = 0;
            if GetConsoleMode(handle, &mut mode) != 0 {
                SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}

struct Executor {
    vm: Arc<VM>,
    ctx: SharedContext,
    current_spans: Option<Arc<Vec<crate::lexer::token::Span>>>,
    fiber_yielded: bool,
    hot_counts: Vec<usize>,
    recording_trace: Option<Trace>,
    is_recording: bool,
    trace_cache: Vec<Option<Arc<Trace>>>,
    http_req: Option<Arc<parking_lot::Mutex<Option<tiny_http::Request>>>>,
    http_req_val: Option<Value>,
}

impl Drop for Executor {
    fn drop(&mut self) {
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn inject_json_into_table(
    table: &mut TableData,
    json: &serde_json::Value,
    mapping: &[(Value, Value)],
) {
    let items: Vec<serde_json::Value> = if let Some(arr) = json.as_array() {
        arr.clone()
    } else {
        vec![json.clone()]
    };
    for item in items {
        let mut new_row = Vec::with_capacity(table.columns.len());
        for col in &table.columns {
            let mut found = false;
            for (k, v) in mapping {
                if k.is_ptr() && (k.0 & 0x000F_0000_0000_0000) == TAG_STR &&
                   v.is_ptr() && (v.0 & 0x000F_0000_0000_0000) == TAG_STR {
                    let col_match = k.as_string();
                    let json_path = v.as_string();
                    if col_match.as_ref() == col.name.as_str() {
                        let pointer = normalize_json_path(json_path.as_ref());
                        let raw = if pointer.is_empty() { item.clone() }
                        else { item.pointer(&pointer).cloned().unwrap_or(serde_json::Value::Null) };
                        let val = json_serde_to_value(&raw);
                        new_row.push(val);
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                new_row.push(Value::from_bool(false));
            }
        }
        table.rows.push(new_row);
    }
}

fn set_op(
    a: &std::collections::BTreeSet<Value>,
    b: &std::collections::BTreeSet<Value>,
    op: u8,
) -> std::collections::BTreeSet<Value> {
    match op {
        0 => { 
            let mut r = a.clone(); 
            for v in b { unsafe { v.inc_ref(); } r.insert(*v); }
            r 
        }
        1 => a.iter().filter(|x| b.contains(x)).map(|x| { unsafe { x.inc_ref(); } *x }).collect(),
        2 => a.iter().filter(|x| !b.contains(x)).map(|x| { unsafe { x.inc_ref(); } *x }).collect(),
        _ => {
            let mut res = std::collections::BTreeSet::new();
            for x in a.iter().filter(|x| !b.contains(x)) { unsafe { x.inc_ref(); } res.insert(*x); }
            for x in b.iter().filter(|x| !a.contains(x)) { unsafe { x.inc_ref(); } res.insert(*x); }
            res
        }
    }
}

enum JoinPred {
    Keys(String, String),
    Lambda(usize),
}

fn join_tables(
    left: &TableData,
    right: &TableData,
    pred: &JoinPred,
    right_name: &str,
    executor: &mut Executor,
) -> TableData {
    let right_key_name: Option<&str> = match pred {
        JoinPred::Keys(_, rk) => Some(rk.as_str()),
        JoinPred::Lambda(_) => None,
    };
    let left_col_names: std::collections::HashSet<&str> =
        left.columns.iter().map(|c| c.name.as_str()).collect();
    let mut out_cols: Vec<VMColumn> = left.columns.clone();
    let mut right_col_map: Vec<Option<usize>> = Vec::new();
    for (_ci, col) in right.columns.iter().enumerate() {
        if right_key_name == Some(col.name.as_str()) {
            right_col_map.push(None);
            continue;
        }
        let out_name = if left_col_names.contains(col.name.as_str()) {
            format!("{}_{}", right_name, col.name)
        } else {
            col.name.clone()
        };
        right_col_map.push(Some(out_cols.len()));
        out_cols.push(VMColumn { name: out_name, ty: col.ty.clone(), is_auto: false });
    }
    let left_rc  = Arc::new(RwLock::new(left.clone()));
    let right_rc = Arc::new(RwLock::new(right.clone()));
    let mut out_rows: Vec<Vec<Value>> = Vec::new();
    for li in 0..left.rows.len() {
        for ri in 0..right.rows.len() {
            let matches = match pred {
                JoinPred::Keys(lk, rk) => {
                    let lc = left.columns.iter().position(|c| &c.name == lk);
                    let rc = right.columns.iter().position(|c| &c.name == rk);
                    match (lc, rc) {
                        (Some(lci), Some(rci)) => left.rows[li][lci] == right.rows[ri][rci],
                        _ => false,
                    }
                }
                JoinPred::Lambda(fid) => {
                    let row_a = Value::from_row(Arc::new(RowRef { table: left_rc.clone(), row_idx: li as u32 }));
                    let row_b = Value::from_row(Arc::new(RowRef { table: right_rc.clone(), row_idx: ri as u32 }));
                    let m = matches!(executor.run_frame(*fid, &[row_a, row_b]), Some(res) if res.is_bool() && res.as_bool());
                    unsafe { row_a.dec_ref(); row_b.dec_ref(); }
                    m
                }
            };
            if matches {
                let mut row = Vec::with_capacity(out_cols.len());
                for v in &left.rows[li] { unsafe { v.inc_ref(); } row.push(*v); }
                for (rci, out_idx) in right_col_map.iter().enumerate() {
                    if let Some(_oi) = out_idx {
                        let v = right.rows[ri][rci];
                        unsafe { v.inc_ref(); }
                        row.push(v);
                    }
                }
                out_rows.push(row);
            }
        }
    }
    TableData { columns: out_cols, rows: out_rows }
}
impl Executor {
    fn current_span_info(&self, ip: usize) -> String {
        if let Some(spans) = &self.current_spans {
            if ip > 0 && ip <= spans.len() {
                let s = &spans[ip - 1];
                return format!(" [line: {}, col: {}]", s.line, s.col);
            }
        }
        "".to_string()
    }


    // ── method dispatch ───────────────────────────────────────────────────────
    fn handle_method_call(&mut self, dst: u8, receiver: Value, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value]) -> OpResult {
        if receiver.is_float() {
            match kind {
                MethodKind::ToStr => {
                    let res = Value::from_string(Arc::new(receiver.as_f64().to_string()));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                    OpResult::Continue
                }
                _ => {
                    eprintln!("Method {:?} not found on Float{}", kind, self.current_span_info(ip));
                    OpResult::Halt
                }
            }
        } else if receiver.is_int() {
            match kind {
                MethodKind::ToStr => {
                    let res = Value::from_string(Arc::new(receiver.as_i64().to_string()));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                    OpResult::Continue
                }
                _ => {
                    eprintln!("Method {:?} not found on Int{}", kind, self.current_span_info(ip));
                    OpResult::Halt
                }
            }
        } else if receiver.is_bool() {
            match kind {
                MethodKind::ToStr => {
                    let res = Value::from_string(Arc::new(receiver.as_bool().to_string()));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                    OpResult::Continue
                }
                _ => {
                    eprintln!("Method {:?} not found on Bool{}", kind, self.current_span_info(ip));
                    OpResult::Halt
                }
            }
        } else if receiver.is_ptr() || receiver.is_date() || receiver.is_func() {
            let tag = receiver.0 & 0x000F_0000_0000_0000;
            match tag {
                TAG_ARR  => self.handle_array_method(dst, receiver.as_array(), kind, args, ip, locals),
                TAG_SET  => self.handle_set_method(dst, receiver.as_set(), kind, args, ip, locals),
                TAG_MAP  => self.handle_map_method(dst, receiver.as_map(), kind, args, ip, locals),
                TAG_TBL  => self.handle_table_method(dst, receiver.as_table(), kind, args, ip, locals),
                TAG_ROW  => {
                    let rr = receiver.as_row();
                    self.handle_row_method(dst, rr, kind, ip, locals)
                }
                TAG_DATE => {
                    let d = chrono::DateTime::from_timestamp_millis(receiver.as_date()).unwrap().with_timezone(&chrono::Local).naive_local();
                    self.handle_date_method(dst, d, kind, args, ip, locals)
                }
                TAG_JSON => self.handle_json_method(dst, receiver.as_json(), kind, args, ip, locals),
                TAG_FIB  => self.handle_fiber_method(dst, receiver.as_fiber(), kind, ip, locals),
                TAG_STR  => self.handle_string_method(dst, &receiver.as_string(), kind, args, ip, locals),
                TAG_FUNC => {
                    eprintln!("Method {:?} not supported for Function type yet{}", kind, self.current_span_info(ip));
                    OpResult::Halt
                }
                _ => {
                    eprintln!("Method {:?} not supported for this pointer type {:x}{}", kind, tag, self.current_span_info(ip));
                    OpResult::Halt
                }
            }
        } else {
            eprintln!("Method call on unknown Value type{}", self.current_span_info(ip));
            OpResult::Halt
        }
    }

    fn handle_method_call_custom(&mut self, dst: u8, receiver: Value, method_name: &str, args: &[Value], ip: usize, locals: &mut [Value]) -> OpResult {
        if receiver.is_ptr() {
            let tag = receiver.0 & 0x000F_0000_0000_0000;
            match tag {
                TAG_ROW  => {
                    let rr = receiver.as_row();
                    self.handle_row_custom(dst, rr, method_name, ip, locals)
                }
                TAG_JSON => self.handle_json_custom(dst, receiver.as_json(), method_name, args, ip, locals),
                _ => {
                    eprintln!("Method {} not found on pointer type {:x}{}", method_name, tag, self.current_span_info(ip));
                    OpResult::Halt
                }
            }
        } else {
            eprintln!("Method {} not found on non-pointer type{}", method_name, self.current_span_info(ip));
            OpResult::Halt
        }
    }

    // ── fiber resume ──────────────────────────────────────────────────────────
    fn resume_fiber(
        &mut self,
        fiber_rc: Arc<RwLock<FiberState>>,
        is_next: bool,
    ) -> Option<Value> {
        let (func_id, mut ip, mut locals) = {
            let mut f = fiber_rc.write();
            if f.is_done { return if is_next { f.yielded_value.clone() } else { None }; }
            (f.func_id, f.ip, std::mem::take(&mut f.locals))
        };
        let chunk = self.ctx.functions[func_id].clone();
        let old_spans = self.current_spans.replace(chunk.spans.clone());
        self.fiber_yielded = false;
        let ores = self.execute_bytecode(&chunk.bytecode, &mut ip, &mut locals);
        let res = match ores {
            OpResult::Return(v) => v,
            OpResult::Yield(v) => v,
            _ => None,
        };
        {
            let mut f = fiber_rc.write();
            f.ip     = ip;
            f.locals = locals;
            if !self.fiber_yielded { f.is_done = true; }
        }
        self.current_spans = old_spans;
        res
    }

    fn execute_trace(&self, trace: &Trace, ip: &mut usize, locals: &mut Vec<Value>, glbs: &mut RwLockWriteGuard<Vec<Value>>) -> Option<OpResult> {
        let native_ptr = trace.native_ptr.load(Ordering::Relaxed);
        if !native_ptr.is_null() {
            let jit_func: crate::backend::jit::JITFunction = unsafe { std::mem::transmute(native_ptr) };
            let result = unsafe { jit_func(locals.as_mut_ptr(), glbs.as_mut_ptr(), self.ctx.constants.as_ptr()) };
            if result == 0 {
                return None;
            } else if result > 0 {
                *ip = result as usize;
                return None;
            } else {
                return Some(OpResult::Halt);
            }
        }

        let mut shutdown_counter: u32 = 0;
        unsafe {
            'trace_loop: loop {
                shutdown_counter += 1;
                if shutdown_counter >= 2048 {
                    shutdown_counter = 0;
                    if SHUTDOWN.load(Ordering::Relaxed) { return Some(OpResult::Halt); }
                }

                for op in &trace.ops {
                    match op {
                        TraceOp::LoadConst { dst, val } => {
                            if val.is_ptr() { val.inc_ref(); }
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = *val;
                        }
                        TraceOp::Move { dst, src } => {
                            let val = *locals.get_unchecked(*src as usize);
                            if val.is_ptr() { val.inc_ref(); }
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = val;
                        }
                        TraceOp::AddInt { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_i64();
                            let b = locals.get_unchecked(*src2 as usize).as_i64();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_i64(a.wrapping_add(b));
                        }

                        TraceOp::SubInt { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_i64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_i64();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_i64(a.wrapping_sub(b_val));
                        }

                        TraceOp::MulInt { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_i64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_i64();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_i64(a.wrapping_mul(b_val));
                        }

                        TraceOp::DivInt { dst, src1, src2, fail_ip } => {
                            let a = locals.get_unchecked(*src1 as usize).as_i64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_i64();
                            if b_val == 0 || (a == i64::MIN && b_val == -1) {
                                *ip = *fail_ip;
                                return None;
                            }
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_i64(a / b_val);
                        }

                        TraceOp::ModInt { dst, src1, src2, fail_ip } => {
                            let a = locals.get_unchecked(*src1 as usize).as_i64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_i64();
                            if b_val == 0 || (a == i64::MIN && b_val == -1) {
                                *ip = *fail_ip;
                                return None;
                            }
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_i64(a % b_val);
                        }

                        TraceOp::AddFloat { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_f64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_f64();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_f64(a + b_val);
                        }
                        TraceOp::SubFloat { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_f64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_f64();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_f64(a - b_val);
                        }
                        TraceOp::MulFloat { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_f64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_f64();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_f64(a * b_val);
                        }
                        TraceOp::DivFloat { dst, src1, src2, fail_ip } => {
                            let a = locals.get_unchecked(*src1 as usize).as_f64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_f64();
                            if b_val == 0.0 {
                                *ip = *fail_ip;
                                return None;
                            }
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_f64(a / b_val);
                        }
                        TraceOp::ModFloat { dst, src1, src2, fail_ip } => {
                            let a = locals.get_unchecked(*src1 as usize).as_f64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_f64();
                            if b_val == 0.0 {
                                *ip = *fail_ip;
                                return None;
                            }
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_f64(a % b_val);
                        }

                        TraceOp::CmpInt { dst, src1, src2, cc } => {
                            let a = locals.get_unchecked(*src1 as usize).as_i64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_i64();
                            let result = match cc {
                                0 => a == b_val,
                                1 => a != b_val,
                                2 => a >  b_val,
                                3 => a <  b_val,
                                4 => a >= b_val,
                                5 => a <= b_val,
                                _ => false,
                            };
                            *locals.get_unchecked_mut(*dst as usize) = Value::from_bool(result);
                        }

                        TraceOp::CmpFloat { dst, src1, src2, cc } => {
                            let a = locals.get_unchecked(*src1 as usize).as_f64();
                            let b_val = locals.get_unchecked(*src2 as usize).as_f64();
                            let result = match cc {
                                0 => a == b_val,
                                1 => a != b_val,
                                2 => a >  b_val,
                                3 => a <  b_val,
                                4 => a >= b_val,
                                5 => a <= b_val,
                                _ => false,
                            };
                            *locals.get_unchecked_mut(*dst as usize) = Value::from_bool(result);
                        }
                        TraceOp::CastIntToFloat { dst, src } => {
                            let a = locals.get_unchecked(*src as usize).as_i64();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_f64(a as f64);
                        }

                        TraceOp::GuardTrue { reg, fail_ip } => {
                            let cond_val = *locals.get_unchecked(*reg as usize);
                            let is_true = cond_val.is_bool() && cond_val.as_bool();
                            if !is_true {
                                *ip = *fail_ip;
                                return None;
                            }
                        }

                        TraceOp::GuardFalse { reg, fail_ip } => {
                            let cond_val = *locals.get_unchecked(*reg as usize);
                            let is_false = cond_val.is_bool() && !cond_val.as_bool();
                            if !is_false {
                                *ip = *fail_ip;
                                return None;
                            }
                        }

                        TraceOp::IncLocal { reg } => {
                            let r = *reg as usize;
                            let val = locals.get_unchecked_mut(r);
                            let v = val.as_i64().wrapping_add(1);
                            *val = Value::from_i64(v);
                        }
                        TraceOp::GetVar { dst, idx } => {
                            let i = *idx as usize;
                            if i < glbs.len() {
                                let val = *glbs.get_unchecked(i);
                                if val.is_ptr() { val.inc_ref(); }
                                let d = *dst as usize;
                                let old = locals.get_unchecked_mut(d);
                                if old.is_ptr() { old.dec_ref(); }
                                *old = val;
                            }
                        }
                        TraceOp::SetVar { idx, src } => {
                            let val = *locals.get_unchecked(*src as usize);
                            if val.is_ptr() { val.inc_ref(); }
                            let i = *idx as usize;
                            if i >= glbs.len() { glbs.resize(i + 1, Value::from_bool(false)); }
                            let target_val = glbs.get_unchecked_mut(i);
                            if target_val.is_ptr() { target_val.dec_ref(); }
                            *target_val = val;
                        }
                        TraceOp::GuardInt { reg, ip: side_exit_ip } => {
                            if !locals.get_unchecked(*reg as usize).is_int() {
                                *ip = *side_exit_ip;
                                return None;
                            }
                        }
                        TraceOp::GuardFloat { reg, ip: side_exit_ip } => {
                            if !locals.get_unchecked(*reg as usize).is_float() {
                                *ip = *side_exit_ip;
                                return None;
                            }
                        }
                        TraceOp::LoopNextInt { reg, limit_reg, target, exit_ip } => {
                            let r = *reg as usize;
                            let limit = locals.get_unchecked(*limit_reg as usize).as_i64();
                            let l_val = locals.get_unchecked_mut(r);

                            if (l_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                let v = (l_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                l_val.0 = (l_val.0 & 0xFFFF_0000_0000_0000) | v;
                                let lv = l_val.as_i64();
                                if lv <= limit {
                                    *ip = *target as usize;
                                    if *ip == trace.start_ip { continue 'trace_loop; }
                                    return None;
                                } else {
                                    *ip = *exit_ip;
                                    return None;
                                }
                            } else {
                                let lv = l_val.as_i64().wrapping_add(1);
                                *l_val = Value::from_i64(lv);
                                if lv <= limit {
                                    *ip = *target as usize;
                                    if *ip == trace.start_ip { continue 'trace_loop; }
                                    return None;
                                } else {
                                    *ip = *exit_ip;
                                    return None;
                                }
                            }
                        }
                        TraceOp::IncVar { g_idx } => {
                            let i = *g_idx as usize;
                            let g_val = glbs.get_unchecked_mut(i);
                            if (g_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                let v = (g_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                g_val.0 = (g_val.0 & 0xFFFF_0000_0000_0000) | v;
                            } else {
                                *g_val = Value::from_i64(g_val.as_i64().wrapping_add(1));
                            }
                        }
                        TraceOp::IncVarLoopNext { g_idx, reg, limit_reg, target, exit_ip } => {
                            let i = *g_idx as usize;
                            let g_val = glbs.get_unchecked_mut(i);
                            
                            if (g_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                let v = (g_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                g_val.0 = (g_val.0 & 0xFFFF_0000_0000_0000) | v;
                            } else {
                                if g_val.is_ptr() { g_val.dec_ref(); }
                                *g_val = Value::from_i64(g_val.as_i64().wrapping_add(1));
                            }

                            let r = *reg as usize;
                            let limit_i64 = locals.get_unchecked(*limit_reg as usize).as_i64();
                            let l_val = locals.get_unchecked_mut(r);
                            
                            if (l_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                let v = (l_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                l_val.0 = (l_val.0 & 0xFFFF_0000_0000_0000) | v;
                                let lv = l_val.as_i64();
                                if lv <= limit_i64 {
                                    *ip = *target as usize;
                                    if *ip == trace.start_ip { continue 'trace_loop; }
                                    return None;
                                } else {
                                    *ip = *exit_ip;
                                    return None;
                                }
                            } else {
                                let lv = l_val.as_i64().wrapping_add(1);
                                *l_val = Value::from_i64(lv);
                                if lv <= limit_i64 {
                                    *ip = *target as usize;
                                    if *ip == trace.start_ip { continue 'trace_loop; }
                                    return None;
                                } else {
                                    *ip = *exit_ip;
                                    return None;
                                }
                            }
                        }
                        TraceOp::IncLocalLoopNext { inc_reg, reg, limit_reg, target, exit_ip } => {
                            let r = *reg as usize;
                            let ir = *inc_reg as usize;

                            let ir_val = locals.get_unchecked_mut(ir);
                            if (ir_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                let v = (ir_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                ir_val.0 = (ir_val.0 & 0xFFFF_0000_0000_0000) | v;
                            } else {
                                *ir_val = Value::from_i64(ir_val.as_i64().wrapping_add(1));
                            }

                            let limit_i64 = locals.get_unchecked(*limit_reg as usize).as_i64();
                            let r_val = locals.get_unchecked_mut(r);
                            
                            let lv = r_val.as_i64().wrapping_add(1);
                            *r_val = Value::from_i64(lv);

                            if lv <= limit_i64 {
                                *ip = *target as usize;
                                if *ip == trace.start_ip { continue 'trace_loop; }
                                return None;
                            } else {
                                *ip = *exit_ip;
                                return None;
                            }
                        }
                        TraceOp::Jump { target_ip } => {
                            *ip = *target_ip;
                            if *ip == trace.start_ip { continue 'trace_loop; }
                            return None;
                        }
                        TraceOp::And { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_bool();
                            let b = locals.get_unchecked(*src2 as usize).as_bool();
                            let d = *dst as usize;
                            *locals.get_unchecked_mut(d) = Value::from_bool(a && b);
                        }
                        TraceOp::Or { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_bool();
                            let b = locals.get_unchecked(*src2 as usize).as_bool();
                            let d = *dst as usize;
                            *locals.get_unchecked_mut(d) = Value::from_bool(a || b);
                        }
                        TraceOp::Not { dst, src } => {
                            let val = locals.get_unchecked(*src as usize).as_bool();
                            let d = *dst as usize;
                            *locals.get_unchecked_mut(d) = Value::from_bool(!val);
                        }
                    }
                }
                return None;
            }
        }
    }
    fn run_frame_owned(&mut self, chunk: FunctionChunk) -> Option<Value> {
        self.current_spans = Some(chunk.spans.clone());
        let mut ip = 0;
        let mut locals = vec![Value::from_bool(false); chunk.max_locals];
        let old_hot = std::mem::replace(&mut self.hot_counts, vec![0; chunk.bytecode.len()]);
        let old_trace_cache = std::mem::replace(&mut self.trace_cache, vec![None; chunk.bytecode.len()]);
        self.recording_trace = None;
        self.is_recording = false;
        {
            let traces = self.vm.traces.read();
            for (ip, trace) in traces.iter() {
                if *ip < self.trace_cache.len() {
                    self.trace_cache[*ip] = Some(trace.clone());
                }
            }
        }
        let res = match self.execute_bytecode(&chunk.bytecode, &mut ip, &mut locals) {
            OpResult::Return(v) => v,
            _ => None,
        };
        self.hot_counts = old_hot;
        self.trace_cache = old_trace_cache;
        for v in locals { unsafe { v.dec_ref(); } }
        res
    }

    fn run_frame(&mut self, func_id: usize, params: &[Value]) -> Option<Value> {
        let chunk = self.ctx.functions[func_id].clone();
        let old_spans = self.current_spans.replace(chunk.spans.clone());
        let mut ip = 0;
        let mut locals = params.to_vec();
        for v in &locals { unsafe { v.inc_ref(); } }
        locals.resize(chunk.max_locals.max(params.len()), Value::from_bool(false));
        let old_hot = std::mem::replace(&mut self.hot_counts, vec![0; chunk.bytecode.len()]);
        let old_trace_cache = std::mem::replace(&mut self.trace_cache, vec![None; chunk.bytecode.len()]);
        self.recording_trace = None;
        self.is_recording = false;
        {
            let traces = self.vm.traces.read();
            for (ip, trace) in traces.iter() {
                if *ip < self.trace_cache.len() {
                    self.trace_cache[*ip] = Some(trace.clone());
                }
            }
        }
        
        let res = match self.execute_bytecode(&chunk.bytecode, &mut ip, &mut locals) {
            OpResult::Return(v) => v,
            _ => None,
        };
        self.hot_counts = old_hot;
        self.trace_cache = old_trace_cache;
        self.current_spans = old_spans;
        for v in locals { unsafe { v.dec_ref(); } }
        res
    }


    fn execute_bytecode(&mut self, bytecode: &[OpCode], ip: &mut usize, locals: &mut Vec<Value>) -> OpResult {
        let vm_arc = self.vm.clone();
        let mut glbs = vm_arc.globals.write();

        while *ip < bytecode.len() {
            if SHUTDOWN.load(Ordering::Relaxed) { return OpResult::Halt; }
            let current_ip = *ip;

            if !self.is_recording && current_ip < self.trace_cache.len() {
                if let Some(trace) = &self.trace_cache[current_ip] {
                    let jit_res = self.execute_trace(trace, ip, locals, &mut glbs);
                    if let Some(res) = jit_res {
                        return res;
                    }
                    continue;
                }
            }

            if let Some(trace) = self.recording_trace.take() {
                self.is_recording = false;
                if current_ip == trace.start_ip && !trace.ops.is_empty() && current_ip < self.trace_cache.len() {
                    let mut jit = self.vm.jit.lock();
                    match jit.compile(&trace) {
                        Ok(ptr) => {
                            trace.native_ptr.store(ptr as *mut u8, Ordering::Relaxed);
                        }
                        Err(e) => {
                        }
                    }
                    drop(jit);
                    let arc_trace = Arc::new(trace);
                    self.vm.traces.write().insert(current_ip, arc_trace.clone());
                    self.trace_cache[current_ip] = Some(arc_trace);
                } else {
                    self.recording_trace = Some(trace);
                    self.is_recording = true;
                }
            }

            let op = bytecode[current_ip];
            *ip += 1;

            if self.is_recording {
                if let Some(ref mut trace) = self.recording_trace {
                match op {
                    OpCode::LoadConst { dst, idx } => {
                        trace.ops.push(TraceOp::LoadConst {
                            dst,
                            val: self.ctx.constants[idx as usize],
                        });
                    }
                    OpCode::Move { dst, src } => {
                        trace.ops.push(TraceOp::Move { dst, src });
                    }
                    OpCode::Add { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::AddInt { dst, src1, src2 });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::AddFloat { dst, src1, src2 });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::Sub { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::SubInt { dst, src1, src2 });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::SubFloat { dst, src1, src2 });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::Mul { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::MulInt { dst, src1, src2 });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::MulFloat { dst, src1, src2 });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }

                    OpCode::Div { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::DivInt { dst, src1, src2, fail_ip: current_ip });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::DivFloat { dst, src1, src2, fail_ip: current_ip });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::Mod { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::ModInt { dst, src1, src2, fail_ip: current_ip });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::ModFloat { dst, src1, src2, fail_ip: current_ip });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::Greater { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpInt { dst, src1, src2, cc: 2 });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpFloat { dst, src1, src2, cc: 2 });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::Less { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpInt { dst, src1, src2, cc: 3 });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpFloat { dst, src1, src2, cc: 3 });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::GreaterEqual { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpInt { dst, src1, src2, cc: 4 });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpFloat { dst, src1, src2, cc: 4 });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::LessEqual { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpInt { dst, src1, src2, cc: 5 });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpFloat { dst, src1, src2, cc: 5 });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::Equal { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpInt { dst, src1, src2, cc: 0 });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpFloat { dst, src1, src2, cc: 0 });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::NotEqual { dst, src1, src2 } => {
                        let a = locals[src1 as usize];
                        let b = locals[src2 as usize];
                        if a.is_int() && b.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpInt { dst, src1, src2, cc: 1 });
                        } else if a.is_float() && b.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                            trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                            trace.ops.push(TraceOp::CmpFloat { dst, src1, src2, cc: 1 });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::JumpIfFalse { src, target } => {
                        let val = locals[src as usize];
                        let taken = val.is_bool() && !val.as_bool();
                        if taken {
                            trace.ops.push(TraceOp::GuardFalse { reg: src, fail_ip: current_ip + 1 });
                        } else {
                            trace.ops.push(TraceOp::GuardTrue { reg: src, fail_ip: target as usize });
                        }
                    }
                    OpCode::JumpIfTrue { src, target } => {
                        let val = locals[src as usize];
                        let taken = val.is_bool() && val.as_bool();
                        if taken {
                            trace.ops.push(TraceOp::GuardTrue { reg: src, fail_ip: current_ip + 1 });
                        } else {
                            trace.ops.push(TraceOp::GuardFalse { reg: src, fail_ip: target as usize });
                        }
                    }
                    OpCode::GetVar { dst, idx } => {
                        trace.ops.push(TraceOp::GetVar { dst, idx });
                    }
                    OpCode::SetVar { idx, src } => {
                        trace.ops.push(TraceOp::SetVar { idx, src });
                    }
                    OpCode::LoopNext { reg, limit_reg, target } => {
                        if locals[reg as usize].is_int() && locals[limit_reg as usize].is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg,       ip: current_ip });
                            trace.ops.push(TraceOp::GuardInt { reg: limit_reg, ip: current_ip });
                            trace.ops.push(TraceOp::LoopNextInt {
                                reg, limit_reg, target, exit_ip: current_ip + 1,
                            });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    OpCode::IncVar { idx } => {
                        trace.ops.push(TraceOp::IncVar { g_idx: idx });
                    }
                    OpCode::IncVarLoopNext { g_idx, reg, limit_reg, target } => {
                        trace.ops.push(TraceOp::IncVarLoopNext {
                            g_idx, reg, limit_reg, target,
                            exit_ip: current_ip + 1,
                        });
                    }
                    OpCode::IncLocal { reg } => {
                        trace.ops.push(TraceOp::IncLocal { reg });
                    }
                    OpCode::IncLocalLoopNext { inc_reg, reg, limit_reg, target } => {
                        trace.ops.push(TraceOp::IncLocalLoopNext {
                            inc_reg, reg, limit_reg, target,
                            exit_ip: current_ip + 1,
                        });
                    }
                    OpCode::Jump { target } => {
                        trace.ops.push(TraceOp::Jump { target_ip: target as usize });
                    }
                    OpCode::And { dst, src1, src2 } => {
                        trace.ops.push(TraceOp::And { dst, src1, src2 });
                    }
                    OpCode::Or { dst, src1, src2 } => {
                        trace.ops.push(TraceOp::Or { dst, src1, src2 });
                    }
                    OpCode::Not { dst, src } => {
                        trace.ops.push(TraceOp::Not { dst, src });
                    }
                    OpCode::CastFloat { dst, src } => {
                        let a = locals[src as usize];
                        if a.is_int() {
                            trace.ops.push(TraceOp::GuardInt { reg: src, ip: current_ip });
                            trace.ops.push(TraceOp::CastIntToFloat { dst, src });
                        } else {
                            self.recording_trace = None;
                            self.is_recording = false;
                        }
                    }
                    _ => {
                        self.recording_trace = None;
                        self.is_recording = false;
                    }
                }
            }
            }

            match op {
                OpCode::LoadConst { dst, idx } => {
                    let val = self.ctx.constants[idx as usize];
                    if val.is_ptr() { unsafe { val.inc_ref(); } }
                    let d = &mut locals[dst as usize];
                    if d.is_ptr() { unsafe { d.dec_ref(); } }
                    *d = val;
                }
                OpCode::Move { dst, src } => {
                    let val = locals[src as usize];
                    if val.is_ptr() { unsafe { val.inc_ref(); } }
                    let d = &mut locals[dst as usize];
                    if d.is_ptr() { unsafe { d.dec_ref(); } }
                    *d = val;
                }
                OpCode::GetVar { dst, idx } => {
                    let val = glbs.get(idx as usize).cloned().unwrap_or(Value::from_bool(false));
                    if val.is_ptr() { unsafe { val.inc_ref(); } }
                    let d = &mut locals[dst as usize];
                    if d.is_ptr() { unsafe { d.dec_ref(); } }
                    *d = val;
                }
                OpCode::SetVar { idx, src } => {
                    let val = locals[src as usize];
                    if val.is_ptr() { unsafe { val.inc_ref(); } }
                    let idx = idx as usize;
                    if idx >= glbs.len() { glbs.resize(idx + 1, Value::from_bool(false)); }
                    let g = &mut glbs[idx];
                    if g.is_ptr() { unsafe { g.dec_ref(); } }
                    *g = val;
                }
                OpCode::Add { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    let res = if a.is_int() && b.is_int() { Value::from_i64(a.as_i64().wrapping_add(b.as_i64())) }
                        else if a.is_numeric() && b.is_numeric() {
                            let f1 = if a.is_int() { a.as_i64() as f64 } else { a.as_f64() };
                            let f2 = if b.is_int() { b.as_i64() as f64 } else { b.as_f64() };
                            Value::from_f64(f1 + f2)
                        }
                        else if a.is_ptr() && (a.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_STR) {
                            Value::from_string(Arc::new(format!("{}{}", a.to_string(), b.to_string())))
                        }
                        else if b.is_ptr() && (b.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_STR) {
                            Value::from_string(Arc::new(format!("{}{}", a.to_string(), b.to_string())))
                        }
                        else if a.is_date() && b.is_int() {
                            Value::from_date(a.as_date() + (b.as_i64() * 86_400_000))
                        }
                        else if a.is_ptr() && (a.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_SET) &&
                                b.is_ptr() && (b.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_SET) {
                            let elements = set_op(&a.as_set().read().elements, &b.as_set().read().elements, 0);
                            Value::from_set(Arc::new(RwLock::new(SetData { elements, cache: None })))
                        }
                        else { Value::from_bool(false) };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Sub { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    let res = if a.is_int() && b.is_int() { Value::from_i64(a.as_i64().wrapping_sub(b.as_i64())) }
                        else if a.is_numeric() && b.is_numeric() {
                            let f1 = if a.is_int() { a.as_i64() as f64 } else { a.as_f64() };
                            let f2 = if b.is_int() { b.as_i64() as f64 } else { b.as_f64() };
                            Value::from_f64(f1 - f2)
                        }
                        else if a.is_date() {
                           if b.is_int() { Value::from_date(a.as_date() - (b.as_i64() * 86_400_000)) }
                           else if b.is_date() { Value::from_i64(a.as_date() - b.as_date()) }
                           else { Value::from_bool(false) }
                        }
                        else if a.is_int() && b.is_date() {
                            Value::from_i64(a.as_i64() - b.as_date())
                        }
                        else if a.is_ptr() && (a.0 & 0x000F_0000_0000_0000) == TAG_SET &&
                                b.is_ptr() && (b.0 & 0x000F_0000_0000_0000) == TAG_SET {
                            let elements = set_op(&a.as_set().read().elements, &b.as_set().read().elements, 2);
                            Value::from_set(Arc::new(RwLock::new(SetData { elements, cache: None })))
                        }
                        else { Value::from_bool(false) };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Mul { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    let res = if a.is_int() && b.is_int() { Value::from_i64(a.as_i64().wrapping_mul(b.as_i64())) }
                        else if a.is_numeric() && b.is_numeric() {
                            let f1 = if a.is_int() { a.as_i64() as f64 } else { a.as_f64() };
                            let f2 = if b.is_int() { b.as_i64() as f64 } else { b.as_f64() };
                            Value::from_f64(f1 * f2)
                        }
                        else if a.is_ptr() && (a.0 & 0x000F_0000_0000_0000) == TAG_SET &&
                                b.is_ptr() && (b.0 & 0x000F_0000_0000_0000) == TAG_SET {
                            let elements = set_op(&a.as_set().read().elements, &b.as_set().read().elements, 1);
                            Value::from_set(Arc::new(RwLock::new(SetData { elements, cache: None })))
                        }
                        else {
                            eprintln!("ERROR: Cannot multiply {} and {}{}", a.to_string(), b.to_string(), self.current_span_info(*ip));
                            return OpResult::Halt;
                        };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Div { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    let res = if a.is_int() && b.is_int() {
                        let vb = b.as_i64();
                        if vb == 0 { eprintln!("R300: Division by zero{}", self.current_span_info(*ip)); return OpResult::Halt; }
                        Value::from_i64(a.as_i64() / vb)
                    }
                    else if a.is_numeric() && b.is_numeric() {
                        let f1 = if a.is_int() { a.as_i64() as f64 } else { a.as_f64() };
                        let f2 = if b.is_int() { b.as_i64() as f64 } else { b.as_f64() };
                        if f2 == 0.0 { eprintln!("R300: Division by zero (float){}", self.current_span_info(*ip)); return OpResult::Halt; }
                        Value::from_f64(f1 / f2)
                    }
                    else { return OpResult::Halt; };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Mod { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    let res = if a.is_int() && b.is_int() {
                        let bv = b.as_i64();
                        if bv != 0 { Value::from_i64(a.as_i64() % bv) } else { Value::from_bool(false) }
                    } else if a.is_numeric() && b.is_numeric() {
                        let f1 = if a.is_int() { a.as_i64() as f64 } else { a.as_f64() };
                        let f2 = if b.is_int() { b.as_i64() as f64 } else { b.as_f64() };
                        Value::from_f64(f1 % f2)
                    } else { Value::from_bool(false) };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Pow { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    let res = if a.is_int() && b.is_int() { Value::from_i64(a.as_i64().pow(b.as_i64() as u32)) }
                        else if a.is_numeric() && b.is_numeric() {
                            let f1 = if a.is_int() { a.as_i64() as f64 } else { a.as_f64() };
                            let f2 = if b.is_int() { b.as_i64() as f64 } else { b.as_f64() };
                            Value::from_f64(f1.powf(f2))
                        } else { Value::from_bool(false) };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Equal { dst, src1, src2 } => {
                    let res = Value::from_bool(locals[src1 as usize] == locals[src2 as usize]);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::NotEqual { dst, src1, src2 } => {
                    let res = Value::from_bool(locals[src1 as usize] != locals[src2 as usize]);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Greater { dst, src1, src2 } => {
                    let res = Value::from_bool(locals[src1 as usize] > locals[src2 as usize]);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Less { dst, src1, src2 } => {
                    let res = Value::from_bool(locals[src1 as usize] < locals[src2 as usize]);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::GreaterEqual { dst, src1, src2 } => {
                    let res = Value::from_bool(locals[src1 as usize] >= locals[src2 as usize]);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::LessEqual { dst, src1, src2 } => {
                    let res = Value::from_bool(locals[src1 as usize] <= locals[src2 as usize]);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::And { dst, src1, src2 } => {
                    let a = locals[src1 as usize].as_bool();
                    let b = locals[src2 as usize].as_bool();
                    let res = Value::from_bool(a && b);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Or { dst, src1, src2 } => {
                    let a = locals[src1 as usize].as_bool();
                    let b = locals[src2 as usize].as_bool();
                    let res = Value::from_bool(a || b);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Not { dst, src } => {
                    let res = Value::from_bool(!locals[src as usize].as_bool());
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Has { dst, src1, src2 } => {
                    let container = locals[src1 as usize];
                    let item = locals[src2 as usize];
                    let res = if container.is_string() && item.is_string() {
                        let c_str = container.to_string();
                        let i_str = item.to_string();
                        Value::from_bool(c_str.contains(&i_str))
                    } else if container.is_array() {
                        let arr_rc = container.as_array();
                        let ok = arr_rc.read().iter().any(|v| v == &item);
                        Value::from_bool(ok)
                    } else if container.is_ptr() && (container.0 & 0x000F_0000_0000_0000) == TAG_SET {
                        let set_rc = container.as_set();
                        let ok = set_rc.read().elements.contains(&item);
                        Value::from_bool(ok)
                    } else if container.is_map() {
                        let map_rc = container.as_map();
                        let ok = map_rc.read().iter().any(|(k, _)| k == &item);
                        Value::from_bool(ok)
                    } else {
                        Value::from_bool(false)
                    };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::Jump { target } => { 
                    let target_ip = target as usize;
                    if target_ip < current_ip {
                        if target_ip < self.hot_counts.len() && !self.is_recording && self.trace_cache[target_ip].is_none() {
                            let count = unsafe { self.hot_counts.get_unchecked_mut(target_ip) };
                            *count += 1;
                            if *count >= 50 {
                                self.recording_trace = Some(Trace { ops: vec![], start_ip: target_ip, native_ptr: AtomicPtr::new(ptr::null_mut()) });
                                self.is_recording = true;
                            }
                        }
                    }
                    *ip = target_ip; 
                    continue; 
                }
                OpCode::JumpIfFalse { src, target } => {
                    let val = locals[src as usize];
                    let ok = if val.is_bool() { val.as_bool() } else { true };
                    if !ok { *ip = target as usize; continue; }
                }
                OpCode::JumpIfTrue { src, target } => {
                    let val = locals[src as usize];
                    let ok = if val.is_bool() { val.as_bool() } else { false };
                    if ok { *ip = target as usize; continue; }
                }
                OpCode::Return { src } => {
                    let val = locals[src as usize];
                    unsafe { val.inc_ref(); }
                    *ip = bytecode.len();
                    return OpResult::Return(Some(val));
                }
                OpCode::ReturnVoid => {
                    *ip = bytecode.len();
                    return OpResult::Return(None);
                }
                OpCode::Yield { src } => {
                    let val = locals[src as usize];
                    unsafe { val.inc_ref(); }
                    self.fiber_yielded = true;
                    return OpResult::Yield(Some(val));
                }
                OpCode::YieldVoid => {
                    self.fiber_yielded = true;
                    return OpResult::Yield(None);
                }
                OpCode::Halt => {
                    return OpResult::Halt;
                }
                OpCode::Print { src } => {
                    println!("{}", locals[src as usize].to_string());
                }
                OpCode::HaltAlert { src } => {
                    println!("ALERT: {}", locals[src as usize].to_string());
                }
                OpCode::HaltError { src } => {
                    eprintln!("ERROR: {}{}", locals[src as usize].to_string(), self.current_span_info(*ip));
                    self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    return OpResult::Halt;
                }
                OpCode::HaltFatal { src } => {
                    eprintln!("FATAL: {}{}", locals[src as usize].to_string(), self.current_span_info(*ip));
                    self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    return OpResult::Halt;
                }
                OpCode::TerminalExit => {
                    std::process::exit(0);
                }
                OpCode::TerminalRun { dst, cmd_src } => {
                    let cmd = locals[cmd_src as usize].to_string();
                    drop(glbs);
                    let status = std::process::Command::new("cargo").args(["run", "--release", "--", &cmd]).status();
                    glbs = vm_arc.globals.write();
                    let success = status.map(|s| s.success()).unwrap_or(false);
                    let res = Value::from_bool(success);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::TerminalClear => {
                    #[cfg(windows)]
                    if let Err(_) = std::process::Command::new("cmd").args(["/c", "cls"]).status() {
                        print!("\x1B[2J\x1B[1;1H");
                    }
                    #[cfg(not(windows))]
                    print!("\x1B[2J\x1B[1;1H");
                    let _ = std::io::stdout().flush();
                }
                OpCode::Call { dst, func_idx, base, arg_count } => {
                    let args = &locals[(base as usize)..(base as usize + arg_count as usize)];
                    drop(glbs);
                    let call_res = self.run_frame(func_idx as usize, args);
                    glbs = vm_arc.globals.write();
                    if self.vm.error_count.load(std::sync::atomic::Ordering::Relaxed) > 0 {
                        return OpResult::Halt;
                    }
                    if let Some(res) = call_res {
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    } else {
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = Value::from_bool(false);
                    }
                }

                OpCode::MethodCall { dst, kind, base, arg_count } => {
                    let receiver = locals[base as usize];
                    let args: Vec<Value> = locals[(base as usize + 1)..(base as usize + 1 + arg_count as usize)].to_vec();
                    drop(glbs);
                    let ores = self.handle_method_call(dst, receiver, kind, &args, *ip, locals);
                    glbs = vm_arc.globals.write();
                    match ores {
                        OpResult::Continue => {}
                        _ => return ores,
                    }
                }
                OpCode::MethodCallCustom { dst, method_name_idx, base, arg_count } => {
                    let receiver = locals[base as usize];
                    let method_name = self.ctx.constants[method_name_idx as usize].as_string();
                    let args: Vec<Value> = locals[(base as usize + 1)..(base as usize + 1 + arg_count as usize)].to_vec();
                    drop(glbs);
                    let ores = self.handle_method_call_custom(dst, receiver, &method_name, &args, *ip, locals);
                    glbs = vm_arc.globals.write();
                    match ores {
                        OpResult::Continue => {}
                        _ => return ores,
                    }
                }
                OpCode::Wait { src } => {
                    let val = locals[src as usize];
                    let ms = if val.is_int() { val.as_i64() as u64 } else if val.is_float() { val.as_f64() as u64 } else { 0 };
                    drop(glbs);
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    glbs = vm_arc.globals.write();
                }
                OpCode::EnvGet { dst, src } => {
                    let name = locals[src as usize].to_string();
                    let res = match std::env::var(&name) {
                        Ok(v) => Value::from_string(Arc::new(v)),
                        Err(_) => Value::from_bool(false),
                    };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::EnvArgs { dst } => {
                    let args: Vec<Value> = std::env::args().map(|s| Value::from_string(Arc::new(s))).collect();
                    let res = Value::from_array(Arc::new(RwLock::new(args)));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::DateNow { dst } => {
                    let now = chrono::Local::now().timestamp_millis();
                    let res = Value::from_date(now);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::JsonParse { dst, src } => {
                    let s = locals[src as usize].to_string();
                    let res = match serde_json::from_str::<serde_json::Value>(&s) {
                        Ok(j) => Value::from_json(Arc::new(RwLock::new(j))),
                        Err(_) => Value::from_bool(false),
                    };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::ArrayInit { dst, base, count } => {
                    let mut elems = Vec::with_capacity(count as usize);
                    for i in 0..count {
                        let v = locals[base as usize + i as usize ];
                        unsafe { v.inc_ref(); }
                        elems.push(v);
                    }
                    let res = Value::from_array(Arc::new(RwLock::new(elems)));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::SetInit { dst, base, count } => {
                    let mut elements = std::collections::BTreeSet::new();
                    for i in 0..count {
                        let v = locals[base as usize + i as usize ];
                        unsafe { v.inc_ref(); }
                        elements.insert(v);
                    }
                    let res = Value::from_set(Arc::new(RwLock::new(SetData { elements, cache: None })));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::MapInit { dst, base, count } => {
                    let mut map = Vec::with_capacity(count as usize);
                    for i in 0..count {
                        let k = locals[base as usize + (i * 2) as usize];
                        let v = locals[base as usize + (i * 2 + 1) as usize];
                        unsafe { k.inc_ref(); v.inc_ref(); }
                        map.push((k, v));
                    }
                    let res = Value::from_map(Arc::new(RwLock::new(map)));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::TableInit { dst, skeleton_idx, base, row_count } => {
                    let skeleton_val = self.ctx.constants[skeleton_idx as usize];
                    if skeleton_val.is_ptr() && (skeleton_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_TBL) {
                        let col_def = skeleton_val.as_table().read().columns.clone();
                        let non_auto_count = col_def.iter().filter(|c| !c.is_auto).count();
                        let mut rows = Vec::with_capacity(row_count as usize);
                        for r in 0..row_count {
                            let mut row_vals = Vec::with_capacity(col_def.len());
                            let mut data_idx = 0;
                            for col in &col_def {
                                if col.is_auto {
                                    row_vals.push(Value::from_i64((r + 1) as i64));
                                } else {
                                    let v = locals[base as usize + (r as usize * non_auto_count) + data_idx];
                                    unsafe { v.inc_ref(); }
                                    row_vals.push(v);
                                    data_idx += 1;
                                }
                            }
                            rows.push(row_vals);
                        }
                        let res = Value::from_table(Arc::new(RwLock::new(TableData { columns: col_def, rows })));
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    }
                }
                OpCode::SetIntersection { dst, src1, src2 } | OpCode::SetDifference { dst, src1, src2 } | OpCode::SetSymDifference { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    if a.is_ptr() && b.is_ptr() && (a.0 & 0x000F_0000_0000_0000) == TAG_SET && (b.0 & 0x000F_0000_0000_0000) == TAG_SET {
                        let op_id = match op {
                            OpCode::SetIntersection { .. } => 1,
                            OpCode::SetDifference { .. }   => 2,
                            _ => 3,
                        };
                        let elements = set_op(&a.as_set().read().elements, &b.as_set().read().elements, op_id);
                        let res = Value::from_set(Arc::new(RwLock::new(SetData { elements, cache: None })));
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    }
                }
                OpCode::RandomChoice { dst, src } => {
                    let col = locals[src as usize];
                    let res = if col.is_ptr() {
                        let mut rng = rand::rng();
                        use rand::Rng;
                        match col.0 & 0x000F_0000_0000_0000 {
                            TAG_ARR => {
                                let a_rc = col.as_array();
                                let arr = a_rc.read();
                                if arr.is_empty() { Value::from_bool(false) }
                                else { let v = arr[rng.random_range(0..arr.len())]; unsafe { v.inc_ref(); } v }
                            }
                            TAG_SET => {
                                let s_rc = col.as_set();
                                {
                                    let s_read = s_rc.read();
                                    if s_read.elements.is_empty() {
                                        Value::from_bool(false)
                                    } else if let Some(ref cache) = s_read.cache {
                                        let v = cache[rng.random_range(0..cache.len())];
                                        unsafe { v.inc_ref(); }
                                        v
                                    } else {
                                        drop(s_read);
                                        let mut s_write = s_rc.write();
                                        if s_write.cache.is_none() {
                                            s_write.cache = Some(s_write.elements.iter().cloned().collect());
                                        }
                                        let cache = s_write.cache.as_ref().unwrap();
                                        let v = cache[rng.random_range(0..cache.len())];
                                        unsafe { v.inc_ref(); }
                                        v
                                    }
                                }
                            }
                            _ => Value::from_bool(false),
                        }
                    } else { Value::from_bool(false) };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::SetRange { dst, start, end, step, has_step } => {
                    let s_val = locals[start as usize];
                    let e_val = locals[end as usize];
                    let st_val = if locals[has_step as usize].as_bool() { locals[step as usize] } else { Value::from_i64(1) };
                    let mut elements = std::collections::BTreeSet::new();
                    if s_val.is_int() && e_val.is_int() && st_val.is_int() {
                        let mut curr = s_val.as_i64();
                        let stop = e_val.as_i64();
                        let inc = st_val.as_i64();
                        if inc > 0 { while curr <= stop { elements.insert(Value::from_i64(curr)); curr += inc; } }
                        else if inc < 0 { while curr >= stop { elements.insert(Value::from_i64(curr)); curr += inc; } }
                    } else {
                        let mut curr = s_val.as_f64();
                        let stop = e_val.as_f64();
                        let inc = if st_val.is_int() { st_val.as_i64() as f64 } else { st_val.as_f64() };
                        if inc > 0.0 {
                            while curr <= stop + 1e-9 {
                                elements.insert(Value::from_f64(curr));
                                curr += inc;
                                if curr > stop + 1e6 { break; }
                            }
                        } else if inc < 0.0 {
                            while curr >= stop - 1e-9 {
                                elements.insert(Value::from_f64(curr));
                                curr += inc;
                                if curr < stop - 1e6 { break; }
                            }
                        }
                    }
                    let res = Value::from_set(Arc::new(RwLock::new(SetData { elements, cache: None })));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }

                OpCode::StoreAppend { base } => {
                    let path_str = locals[base as usize].to_string();
                    let path = std::path::Path::new(&path_str);
                    let content = locals[(base + 1) as usize].to_string();
                    use std::io::Write as _;
                    drop(glbs);
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(path) {
                        let _ = write!(f, "{}", content);
                    }
                    glbs = vm_arc.globals.write();
                }
                OpCode::StoreExists { dst, base } => {
                    let path = locals[base as usize].to_string();
                    let res = Value::from_bool(std::path::Path::new(&path).exists());
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::StoreDelete { base } => {
                    let path = locals[base as usize].to_string();
                    let _ = std::fs::remove_file(path);
                }
                OpCode::JsonBindLocal { dst, json_src, path_src } => {
                    let json_val = locals[json_src as usize];
                    let path_val = locals[path_src as usize];
                    if json_val.is_ptr() && (json_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_JSON) {
                        let path = path_val.to_string();
                        let json_arc = json_val.as_json();
                        let mut j = json_arc.write();
                        let res = j.pointer_mut(&normalize_json_path(&path)).cloned().unwrap_or(serde_json::Value::Null);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = json_serde_to_value(&res);
                    }
                }
                OpCode::JsonBind { idx, json_src, path_src } => {
                    let json_val = locals[json_src as usize];
                    let path_val = locals[path_src as usize];
                    if json_val.is_ptr() && (json_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_JSON) {
                        let path = path_val.to_string();
                        let val = {
                            let json_arc = json_val.as_json();
                            let mut j = json_arc.write();
                            let serde_res = j.pointer_mut(&normalize_json_path(&path)).cloned().unwrap_or(serde_json::Value::Null);
                            json_serde_to_value(&serde_res)
                        };
                        let idx = idx as usize;
                        if idx >= glbs.len() { glbs.resize(idx + 1, Value::from_bool(false)); }
                        unsafe { glbs[idx].dec_ref(); }
                        glbs[idx] = val;
                    }
                }
                OpCode::JsonInjectLocal { table_reg, json_src, mapping_src } => {
                    let table = locals[table_reg as usize];
                    let json = locals[json_src as usize];
                    let mapping = locals[mapping_src as usize];
                    self.json_inject_table(&table, &json, &mapping);
                }
                OpCode::JsonInject { table_idx, json_src, mapping_src } => {
                    let table = glbs[table_idx as usize];
                    let json = locals[json_src as usize];
                    let mapping = locals[mapping_src as usize];
                    self.json_inject_table(&table, &json, &mapping);
                }
                OpCode::IncLocalLoopNext { inc_reg, reg, limit_reg, target } => {
                    let r_idx = reg as usize;
                    let l_idx = limit_reg as usize;
                    let limit_i64 = unsafe { locals.get_unchecked(l_idx).as_i64() };
                    let val = unsafe { *locals.get_unchecked(r_idx) };
                    
                    if val.is_int() {
                        let next = val.as_i64().wrapping_add(1);
                        unsafe { *locals.get_unchecked_mut(r_idx) = Value::from_i64(next); }
                        
                        let i_idx = inc_reg as usize;
                        let i_val = unsafe { locals.get_unchecked_mut(i_idx) };
                        if (i_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                            let v = (i_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                            i_val.0 = (i_val.0 & 0xFFFF_0000_0000_0000) | v;
                        } else {
                            *i_val = Value::from_i64(i_val.as_i64().wrapping_add(1));
                        }

                        if next <= limit_i64 {
                            if SHUTDOWN.load(Ordering::Relaxed) { return OpResult::Halt; }
                            let target_ip = target as usize;
                            if target_ip < self.hot_counts.len() {
                                let hc = unsafe { self.hot_counts.get_unchecked_mut(target_ip) };
                                *hc += 1;
                                if *hc >= 50 && !self.is_recording && self.trace_cache[target_ip].is_none() {
                                    self.recording_trace = Some(Trace { ops: vec![], start_ip: target_ip, native_ptr: AtomicPtr::new(ptr::null_mut()) });
                                    self.is_recording = true;
                                }
                            }
                            *ip = target_ip;
                            continue;
                        }
                    }
                }
                OpCode::IncVarLoopNext { g_idx, reg, limit_reg, target } => {
                    let r_idx = reg as usize;
                    let l_idx = limit_reg as usize;
                    let limit_i64 = unsafe { locals.get_unchecked(l_idx).as_i64() };
                    let l_val = unsafe { locals.get_unchecked_mut(r_idx) };
                    
                    if (l_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                        let next_v = (l_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                        l_val.0 = (l_val.0 & 0xFFFF_0000_0000_0000) | next_v;
                        let next = l_val.as_i64();
                        
                        let g_idx = g_idx as usize;
                        if g_idx < glbs.len() {
                            let g_val = unsafe { glbs.get_unchecked_mut(g_idx) };
                            if (g_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                let v = (g_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                g_val.0 = (g_val.0 & 0xFFFF_0000_0000_0000) | v;
                            } else {
                                if g_val.is_ptr() { unsafe { g_val.dec_ref(); } }
                                *g_val = Value::from_i64(g_val.as_i64().wrapping_add(1));
                            }
                        }
                        if next <= limit_i64 {
                            if SHUTDOWN.load(Ordering::Relaxed) { return OpResult::Halt; }
                            let target_ip = target as usize;
                            if target_ip < self.hot_counts.len() {
                                let hc = unsafe { self.hot_counts.get_unchecked_mut(target_ip) };
                                *hc += 1;
                                if *hc >= 50 && !self.is_recording && self.trace_cache[target_ip].is_none() {
                                    self.recording_trace = Some(Trace { ops: vec![], start_ip: target_ip, native_ptr: AtomicPtr::new(ptr::null_mut()) });
                                    self.is_recording = true;
                                }
                            }
                            *ip = target_ip;
                            continue;
                        }
                    } else if l_val.is_int() {
                        let next = l_val.as_i64().wrapping_add(1);
                        *l_val = Value::from_i64(next);
                        let g_idx = g_idx as usize;
                        if g_idx < glbs.len() {
                            let g_val = unsafe { glbs.get_unchecked_mut(g_idx) };
                            if (g_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                let v = (g_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                g_val.0 = (g_val.0 & 0xFFFF_0000_0000_0000) | v;
                            } else {
                                if g_val.is_ptr() { unsafe { g_val.dec_ref(); } }
                                *g_val = Value::from_i64(g_val.as_i64().wrapping_add(1));
                            }
                        }
                        if next <= limit_i64 {
                            if SHUTDOWN.load(Ordering::Relaxed) { return OpResult::Halt; }
                            let target_ip = target as usize;
                            if target_ip < self.hot_counts.len() {
                                let hc = unsafe { self.hot_counts.get_unchecked_mut(target_ip) };
                                *hc += 1;
                                if *hc >= 50 && !self.is_recording && self.trace_cache[target_ip].is_none() {
                                    self.recording_trace = Some(Trace { ops: vec![], start_ip: target_ip, native_ptr: AtomicPtr::new(ptr::null_mut()) });
                                    self.is_recording = true;
                                }
                            }
                            *ip = target_ip;
                            continue;
                        }
                    }
                }
                OpCode::IncVar { idx } => {
                    let idx = idx as usize;
                    if idx < glbs.len() {
                        let val = unsafe { glbs.get_unchecked_mut(idx) };
                        if (val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                            let next = (val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                            val.0 = (val.0 & 0xFFFF_0000_0000_0000) | next;
                        } else {
                            if val.is_ptr() { unsafe { val.dec_ref(); } }
                            *val = Value::from_i64(val.as_i64().wrapping_add(1));
                        }
                    }
                }
                OpCode::SetUnion { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    if a.is_ptr() && b.is_ptr() && (a.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_SET) && (b.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_SET) {
                        let elements = set_op(&a.as_set().read().elements, &b.as_set().read().elements, 0);
                        let res = Value::from_set(Arc::new(RwLock::new(SetData { elements, cache: None })));
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    }
                }
                OpCode::IntConcat { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    let s = format!("{}{}", a.to_string(), b.to_string());
                    let res = if let Ok(i) = s.parse::<i64>() { Value::from_i64(i) } else { Value::from_string(Arc::new(s)) };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::CryptoHash { dst, pass_src, alg_src } => {
                    let password = locals[pass_src as usize].to_string();
                    let algo = locals[alg_src as usize].to_string();
                    drop(glbs);
                    let res = match algo.as_str() {
                        "bcrypt" => bcrypt::hash(&password, bcrypt::DEFAULT_COST).map(|h| Value::from_string(Arc::new(h))).unwrap_or(Value::from_bool(false)),
                        "argon2" => {
                            let mut salt_bytes = [0u8; 16];
                            rand::fill(&mut salt_bytes);
                            let salt = SaltString::encode_b64(&salt_bytes).unwrap();
                            let argon2 = argon2::Argon2::default();
                            argon2.hash_password(password.as_bytes(), &salt)
                                .map(|h| Value::from_string(Arc::new(h.to_string())))
                                .unwrap_or(Value::from_bool(false))
                        }
                        _ => Value::from_bool(false),
                    };
                    glbs = vm_arc.globals.write();
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::CryptoVerify { dst, pass_src, hash_src, alg_src } => {
                    let password = locals[pass_src as usize].to_string();
                    let hashed = locals[hash_src as usize].to_string();
                    let algo = locals[alg_src as usize].to_string();
                    drop(glbs);
                    let ok = match algo.as_str() {
                        "bcrypt" => bcrypt::verify(&password, &hashed).unwrap_or(false),
                        "argon2" => {
                            if let Ok(parsed_hash) = PasswordHash::new(&hashed) {
                                argon2::Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
                            } else { false }
                        }
                        _ => false,
                    };
                    glbs = vm_arc.globals.write();
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(ok);
                }
                OpCode::CryptoToken { dst, len_src } => {
                    let len = locals[len_src as usize].as_i64() as usize;
                    let token: String = (0..len).map(|_| {
                        const CHARSET: &[u8] = b"0123456789abcdef";
                        let idx = rand::rng().random_range(0..CHARSET.len());
                        CHARSET[idx] as char
                    }).collect();
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_string(Arc::new(token));
                }
                OpCode::CastInt { dst, src } => {
                    let val = locals[src as usize];
                    let res = if val.is_int() { val } else if val.is_float() { Value::from_i64(val.as_f64() as i64) } else { Value::from_i64(0) };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::CastFloat { dst, src } => {
                    let val = locals[src as usize];
                    let res = if val.is_float() { val } else if val.is_int() { Value::from_f64(val.as_i64() as f64) } else { Value::from_f64(0.0) };
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::CastString { dst, src } => {
                    let res = Value::from_string(Arc::new(locals[src as usize].to_string()));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::CastBool { dst, src } => {
                    let val = locals[src as usize];
                    let res = Value::from_bool(val.as_bool());
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::IncLocal { reg } => {
                    let val = locals[reg as usize];
                    if val.is_int() {
                        locals[reg as usize] = Value::from_i64(val.as_i64().wrapping_add(1));
                    }
                }
                OpCode::LoopNext { reg, limit_reg, target } => {
                    let val = locals[reg as usize];
                    let limit = locals[limit_reg as usize];
                    if val.is_int() && limit.is_int() {
                        let v = val.as_i64().wrapping_add(1);
                        locals[reg as usize] = Value::from_i64(v);
                        if v <= limit.as_i64() {
                            let target_ip = target as usize;
                            if target_ip < self.hot_counts.len() {
                                self.hot_counts[target_ip] += 1;
                                if self.hot_counts[target_ip] >= 50 && !self.is_recording && self.trace_cache[target_ip].is_none() {
                                    self.recording_trace = Some(Trace { ops: vec![], start_ip: target_ip, native_ptr: AtomicPtr::new(ptr::null_mut()) });
                                    self.is_recording = true;
                                }
                            }
                            *ip = target_ip;
                            continue;
                        }
                    } else {
                        eprintln!("ERROR: LoopNext on non-integers{}", self.current_span_info(current_ip));
                        return OpResult::Halt;
                    }
                }
                OpCode::FiberCreate { dst, func_idx, base, arg_count } => {
                    let chunk = self.ctx.functions[func_idx as usize].clone();
                    let args = &locals[(base as usize)..(base as usize + arg_count as usize)];
                    let mut fiber_locals = args.to_vec();
                    fiber_locals.resize(chunk.max_locals, Value::from_bool(false));
                    let f = FiberState {
                        func_id: func_idx as usize,
                        ip: 0,
                        locals: fiber_locals,
                        yielded_value: None,
                        is_done: false,
                    };
                    for v in &f.locals { unsafe { v.inc_ref(); } }
                    let res = Value::from_fiber(Arc::new(RwLock::new(f)));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpCode::HttpCall { dst, method_idx, url_src, body_src } => {
                    let url = locals[url_src as usize].to_string();
                    let body = locals[body_src as usize].to_string();
                    let method = self.ctx.constants[method_idx as usize].to_string();
                    
                    if url.contains("169.254.") || url.contains("instance-data") {
                        let mut map = serde_json::Map::new();
                        map.insert("ok".to_string(), serde_json::Value::Bool(false));
                        map.insert("error".to_string(), serde_json::Value::String("SSRF attempt blocked".to_string()));
                        let res = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(map))));
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                        continue;
                    }
                    
                    drop(glbs);
                    let res = match method.to_uppercase().as_str() {
                        "GET" => ureq::get(&url).call(),
                        "POST" => ureq::post(&url).send_string(&body),
                        _ => ureq::get(&url).call(),
                    };
                    glbs = vm_arc.globals.write();
                    let val = Value::from_json(Arc::new(RwLock::new(build_response_json(res))));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = val;
                }
                OpCode::HttpRequest { dst, arg_src } => {
                    let arg_val = locals[arg_src as usize];
                    if arg_val.is_map() {
                        let map_rc = arg_val.as_map();
                        let map = map_rc.read();
                        
                        let method = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == "method").map(|(_, v)| v.to_string()).unwrap_or_else(|| "GET".to_string());
                        let url = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == "url").map(|(_, v)| v.to_string()).unwrap_or_default();
                        
                        if let Err(e) = is_safe_url(&url) {
                            let mut res_map = serde_json::Map::new();
                            res_map.insert("ok".to_string(), serde_json::Value::Bool(false));
                            res_map.insert("error".to_string(), serde_json::Value::String(e));
                            let val = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(res_map))));
                            unsafe { locals[dst as usize].dec_ref(); }
                            locals[dst as usize] = val;
                        } else {
                            drop(map);
                            drop(glbs);
                            
                            let mut request = match method.to_uppercase().as_str() {
                                "POST" => ureq::post(&url),
                                "PUT" => ureq::put(&url),
                                "DELETE" => ureq::delete(&url),
                                "PATCH" => ureq::patch(&url),
                                "HEAD" => ureq::head(&url),
                                _ => ureq::get(&url),
                            };
                            
                            let map = map_rc.read();
                            if let Some((_, h_val)) = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == "headers") {
                                if h_val.is_map() {
                                    let h_map_rc = h_val.as_map();
                                    let h_map = h_map_rc.read();
                                    for (k, v) in h_map.iter() {
                                        request = request.set(&k.to_string(), &v.to_string());
                                    }
                                }
                            }
                            
                            if let Some((_, t_val)) = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == "timeout") {
                                if t_val.is_int() {
                                    request = request.timeout(std::time::Duration::from_millis(t_val.as_i64() as u64));
                                }
                            }
                            
                            let body_val = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == "body").map(|(_, v)| *v);
                            let response = if let Some(b) = body_val {
                                request.send_string(&b.to_string())
                            } else {
                                request.call()
                            };
                            
                            glbs = vm_arc.globals.write();
                            let val = Value::from_json(Arc::new(RwLock::new(build_response_json(response))));
                            unsafe { locals[dst as usize].dec_ref(); }
                            locals[dst as usize] = val;
                        }
                    } else {
                        let res = self.http_req_val.unwrap_or(Value::from_bool(false));
                        unsafe { res.inc_ref(); }
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    }
                }
                OpCode::HttpRespond { status_src, body_src, headers_src } => {
                    let status = locals[status_src as usize].as_i64() as u32;
                    let body_val = locals[body_src as usize];
                    let body = if body_val.is_ptr() && (body_val.0 & 0x000F_0000_0000_0000) == TAG_JSON {
                        body_val.as_json().read().to_string()
                    } else if body_val.is_ptr() && (body_val.0 & 0x000F_0000_0000_0000) == TAG_STR {
                        body_val.as_string().to_string()
                    } else if body_val.is_ptr() && (body_val.0 & 0x000F_0000_0000_0000) == TAG_ARR {
                         value_to_json(&body_val).to_string()
                    } else if body_val.is_ptr() && (body_val.0 & 0x000F_0000_0000_0000) == TAG_MAP {
                         value_to_json(&body_val).to_string()
                    } else if body_val.is_ptr() && (body_val.0 & 0x000F_0000_0000_0000) == TAG_TBL {
                         value_to_json(&body_val).to_string()
                    } else {
                        body_val.to_string()
                    };
                    let headers = locals[headers_src as usize];
                    
                    if let Some(req_mutex_arc) = self.http_req.clone() {
                        let mut req_opt = req_mutex_arc.lock();
                        if let Some(request) = req_opt.take() {
                            let mut response = tiny_http::Response::from_string(body)
                                .with_status_code(status);
                            
                            let mut ct_set = false;
                            
                            if headers.is_array() {
                                let arr_rc = headers.as_array();
                                let arr = arr_rc.read();
                                for item in arr.iter() {
                                    if item.is_map() {
                                        let map_rc = item.as_map();
                                        let map = map_rc.read();
                                        for (k, v) in map.iter() {
                                            let ks = k.to_string();
                                            let vs = v.to_string();
                                            if ks.to_lowercase() == "content-type" { ct_set = true; }
                                            if let Ok(h) = tiny_http::Header::from_bytes(ks.as_bytes(), vs.as_bytes()) {
                                                response = response.with_header(h);
                                            }
                                        }
                                    }
                                }
                            }
                            
                            if !ct_set {
                                response = response.with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                            }
                            
                            response = response.with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                            response = response.with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS, DELETE, PATCH"[..]).unwrap());
                            response = response.with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type, Authorization, X-CSRF-TOKEN"[..]).unwrap());
                            
                            let _ = request.respond(response);
                        }
                    }
                    return OpResult::Yield(None);
                }
                OpCode::HttpServe { func_idx: _, port_src, host_src, workers_src, routes_src } => {
                    let port = locals[port_src as usize].as_i64() as u16;
                    let host = locals[host_src as usize].to_string();
                    let workers = locals[workers_src as usize].as_i64() as usize;
                    let routes_val = locals[routes_src as usize];
                    
                    let addr = format!("{}:{}", host, port);
                    let server = Arc::new(tiny_http::Server::http(&addr).expect("Failed to start server"));
                    let mut routes = Vec::new();
                    if routes_val.is_array() {
                        let arr_rc = routes_val.as_array();
                        let arr = arr_rc.read();
                        for item in arr.iter() {
                            if item.is_map() {
                                let map_rc = item.as_map();
                                let map = map_rc.read();
                                if let Some((k, v)) = map.iter().next() {
                                    if v.is_func() {
                                        let fid = v.as_function() as usize;
                                        routes.push((k.to_string(), fid));
                                    }
                                }
                            }
                        }
                    } else if routes_val.is_map() {
                        let map_rc = routes_val.as_map();
                        let map = map_rc.read();
                        for (k, v) in map.iter() {
                            if v.is_func() {
                                let fid = v.as_function() as usize;
                                routes.push((k.to_string(), fid));
                            }
                        }
                    }

                    
                    drop(glbs);
                    let routes = Arc::new(routes);
                    for _ in 0..workers {
                        let server = server.clone();
                        let routes = routes.clone();
                        let vm = vm_arc.clone();
                        let ctx = self.ctx.clone();
                        std::thread::spawn(move || {
                            while !SHUTDOWN.load(Ordering::Relaxed) {
                                if let Ok(Some(mut request)) = server.recv_timeout(std::time::Duration::from_millis(100)) {
                                    let method = request.method().to_string();
                                    let url = request.url().to_string();
                                    let route_key = format!("{} {}", method, url).to_lowercase();


                                    if method == "OPTIONS" {
                                        let response = tiny_http::Response::from_string("")
                                            .with_status_code(204)
                                            .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap())
                                            .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS, DELETE, PATCH"[..]).unwrap())
                                            .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type, Authorization, X-CSRF-TOKEN"[..]).unwrap());
                                        let _ = request.respond(response);
                                        continue;
                                    }
                                    
                                    let handler_idx = routes.iter()
                                        .find(|(r, _)| {
                                            let r_norm = r.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
                                            let rk_norm = route_key.split_whitespace().collect::<Vec<_>>().join(" ");
                                            r_norm == rk_norm || *r == "*" 
                                        })
                                        .map(|(_, idx)| *idx);
                                    
                                    if let Some(fid) = handler_idx {
                                        let mut body = String::new();
                                        let _ = request.as_reader().read_to_string(&mut body);
                                        
                                        let mut req_map = serde_json::Map::new();
                                        req_map.insert("method".into(), method.into());
                                        req_map.insert("url".into(), url.into());
                                        let body_json: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::String(body));
                                        req_map.insert("body".into(), body_json);
                                        let mut ip_str = request.remote_addr().map(|a| a.ip().to_string()).unwrap_or_default();
                                        if ip_str == "::1" { ip_str = "127.0.0.1".to_string(); }
                                        req_map.insert("ip".into(), ip_str.into());
                                        
                                        let mut headers_map = serde_json::Map::new();
                                        for h in request.headers() {
                                            headers_map.insert(h.field.to_string().to_lowercase(), h.value.to_string().into());
                                        }
                                        req_map.insert("headers".into(), serde_json::Value::Object(headers_map));
                                        
                                        let req_val = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(req_map))));
                                        unsafe { req_val.inc_ref(); } 
                                        
                                        let req_mutex_arc = Arc::new(Mutex::new(Some(request)));
                                         let mut sub_executor = Executor {
                                            vm: vm.clone(),
                                            ctx: ctx.clone(), 
                                            current_spans: None,
                                            fiber_yielded: false,
                                            hot_counts: vec![0; 1024],
                                            recording_trace: None,
                                            is_recording: false,
                                            trace_cache: vec![None; 1024],
                                            http_req: Some(req_mutex_arc.clone()),
                                            http_req_val: Some(req_val),
                                         };
                                         
                                         let chunk = sub_executor.ctx.functions[fid].clone();
                                         let mut ip = 0;
                                         let mut locals = vec![req_val];
                                         unsafe { req_val.inc_ref(); } 
                                         locals.resize(chunk.max_locals.max(1), Value::from_bool(false));
                                         
                                         let ores = sub_executor.execute_bytecode(&chunk.bytecode, &mut ip, &mut locals);

                                         let mut req_opt = req_mutex_arc.lock();
                                         if let Some(req) = req_opt.take() {
                                             let response = tiny_http::Response::from_string("Internal Server Error (No Response Sent)")
                                                 .with_status_code(500)
                                                 .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap())
                                                 .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/plain"[..]).unwrap());
                                             let _ = req.respond(response);
                                         }

                                         for v in locals { unsafe { v.dec_ref(); } }
                                         unsafe { req_val.dec_ref(); }
                                    } else {
                                        let response = tiny_http::Response::from_string("Not Found")
                                            .with_status_code(404)
                                            .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap())
                                            .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS, DELETE, PATCH"[..]).unwrap())
                                            .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type, Authorization, X-CSRF-TOKEN"[..]).unwrap());
                                        let _ = request.respond(response);
                                    }
                                }
                            }
                        });
                    }
                    
                    while !SHUTDOWN.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                    return OpResult::Halt;
                }
                OpCode::StoreWrite { base } => {
                    let path_str = locals[base as usize].to_string();
                    let path = std::path::Path::new(&path_str);
                    let content = locals[(base + 1) as usize].to_string();
                    drop(glbs);
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(path, content);
                    glbs = vm_arc.globals.write();
                }
                OpCode::StoreRead { dst, base } => {
                    let path = locals[base as usize].to_string();
                    drop(glbs);
                    let res = std::fs::read_to_string(path).map(|s| Value::from_string(Arc::new(s))).unwrap_or(Value::from_bool(false));
                    glbs = vm_arc.globals.write();
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                _ => {
                    eprintln!("Halted: Opcode {:?} not yet implemented for register VM{}", op, self.current_span_info(*ip));
                    return OpResult::Halt;
                }
            }
        }
        OpResult::Continue
    }

    fn json_inject_table(&mut self, table_val: &Value, json_val: &Value, mapping_val: &Value) {
        if !table_val.is_ptr() || (table_val.0 & 0x000F_0000_0000_0000) != TAG_TBL { return; }
        if !json_val.is_ptr() || (json_val.0 & 0x000F_0000_0000_0000) != TAG_JSON { return; }
        if !mapping_val.is_ptr() || (mapping_val.0 & 0x000F_0000_0000_0000) != TAG_MAP { return; }

        let table_rc = table_val.as_table();
        let mut table = table_rc.write();
        let json_rc = json_val.as_json();
        let json = json_rc.read();
        let mapping_rc = mapping_val.as_map();
        let mapping = mapping_rc.read();
        
        inject_json_into_table(&mut table, &json, &mapping);
    }

    fn handle_array_method(&mut self, dst: u8, arr_rc: Arc<RwLock<Vec<Value>>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value]) -> OpResult {
        match kind {
            MethodKind::Push => { 
                let val = args[0];
                unsafe { val.inc_ref(); }
                arr_rc.write().push(val); 
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Pop  => { 
                let res = arr_rc.write().pop().unwrap_or(Value::from_bool(false)); 
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Len | MethodKind::Count | MethodKind::Size => {
                let res = Value::from_i64(arr_rc.read().len() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Clear => { 
                let mut arr = arr_rc.write();
                for v in arr.drain(..) { unsafe { v.dec_ref(); } }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Contains => {
                let res = Value::from_bool(arr_rc.read().contains(&args[0]));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::IsEmpty  => {
                let res = Value::from_bool(arr_rc.read().is_empty());
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Get => {
                let arr = arr_rc.read();
                if args[0].is_int() {
                    let i = args[0].as_i64();
                    if i >= 0 && (i as usize) < arr.len() {
                        let v = arr[i as usize];
                        unsafe { v.inc_ref(); }
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = v;
                    } else {
                        eprintln!("R303: Array index out of bounds: {}{}", i, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else { 
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Insert => {
                if args[0].is_int() {
                    let i = args[0].as_i64();
                    let val = args[1];
                    let mut arr = arr_rc.write();
                    if i >= 0 && (i as usize) <= arr.len() {
                        unsafe { val.inc_ref(); }
                        arr.insert(i as usize, val);
                        let res = Value::from_bool(true);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    } else {
                        eprintln!("R303: Array insert index out of bounds: {}{}", i, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else { 
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Update => {
                if args[0].is_int() {
                    let i = args[0].as_i64();
                    let val = args[1];
                    let mut arr = arr_rc.write();
                    if i >= 0 && (i as usize) < arr.len() {
                        unsafe { val.inc_ref(); }
                        let old = arr[i as usize];
                        arr[i as usize] = val;
                        unsafe { old.dec_ref(); }
                        let res = Value::from_bool(true);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    } else {
                        eprintln!("R303: Array update index out of bounds: {}{}", i, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else { 
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Delete => {
                if args[0].is_int() {
                    let i = args[0].as_i64();
                    let mut arr = arr_rc.write();
                    if i >= 0 && (i as usize) < arr.len() {
                        let old = arr.remove(i as usize);
                        unsafe { old.dec_ref(); }
                        let res = Value::from_bool(true);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    } else {
                        eprintln!("R303: Array delete index out of bounds: {}{}", i, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else { 
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Find => {
                let needle = &args[0];
                let arr = arr_rc.read();
                let idx = arr.iter().position(|v| v == needle).map(|i| i as i64).unwrap_or(-1);
                let res = Value::from_i64(idx);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Join => {
                let sep = if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_STR {
                    let s = args[0].as_string();
                    (*s).clone()
                } else { "".to_string() };
                let arr = arr_rc.read();
                let res_str = arr.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(&sep);
                let res = Value::from_string(Arc::new(res_str));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Show => { 
                let arr_val = Value::from_array(arr_rc.clone());
                println!("{}", arr_val.to_string()); 
                unsafe { arr_val.dec_ref(); }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Sort => {
                arr_rc.write().sort();
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Reverse => {
                arr_rc.write().reverse();
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => { eprintln!("Method {:?} not supported for Array{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_set_method(&mut self, dst: u8, set_rc: Arc<RwLock<SetData>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value]) -> OpResult {
        let mut set_data = set_rc.write();
        match kind {
            MethodKind::Add => { 
                let val = args[0];
                unsafe { val.inc_ref(); }
                if set_data.elements.insert(val) {
                    set_data.cache = None;
                }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Remove   => { 
                let res_bool = set_data.elements.remove(&args[0]);
                if res_bool {
                    set_data.cache = None;
                }
                let res = Value::from_bool(res_bool);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Has | MethodKind::Contains => {
                let res = Value::from_bool(set_data.elements.contains(&args[0]));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Len | MethodKind::Count | MethodKind::Size => {
                let res = Value::from_i64(set_data.elements.len() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Clear => { 
                for v in set_data.elements.iter() { unsafe { v.dec_ref(); } }
                set_data.elements.clear();
                set_data.cache = None;
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::IsEmpty  => {
                let res = Value::from_bool(set_data.elements.is_empty());
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Values => {
                let mut vec = Vec::with_capacity(set_data.elements.len());
                for v in set_data.elements.iter() {
                    unsafe { v.inc_ref(); }
                    vec.push(*v);
                }
                let res = Value::from_array(Arc::new(RwLock::new(vec)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Show     => { 
                drop(set_data);
                let set_val = Value::from_set(set_rc.clone());
                println!("{}", set_val.to_string()); 
                unsafe { set_val.dec_ref(); }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => { eprintln!("Method {:?} not supported for Set{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_string_method(&mut self, dst: u8, s: &str, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value]) -> OpResult {
        match kind {
            MethodKind::Length | MethodKind::Size => {
                let res = Value::from_i64(s.chars().count() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Upper  => {
                let res = Value::from_string(Arc::new(s.to_uppercase()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Lower  => {
                let res = Value::from_string(Arc::new(s.to_lowercase()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Trim   => {
                let res = Value::from_string(Arc::new(s.trim().to_string()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::IndexOf => {
                let res = if let Some(v) = args.first() {
                    if v.is_ptr() && (v.0 & 0x000F_0000_0000_0000) == TAG_STR {
                        let sub = v.as_string();
                        let idx = s.find(sub.as_ref()).map(|i| i as i64).unwrap_or(-1);
                        Value::from_i64(idx)
                    } else { Value::from_i64(-1) }
                } else { Value::from_i64(-1) };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::LastIndexOf => {
                let res = if let Some(v) = args.first() {
                    if v.is_ptr() && (v.0 & 0x000F_0000_0000_0000) == TAG_STR {
                        let sub = v.as_string();
                        let idx = s.rfind(sub.as_ref()).map(|i| i as i64).unwrap_or(-1);
                        Value::from_i64(idx)
                    } else { Value::from_i64(-1) }
                } else { Value::from_i64(-1) };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Replace => {
                if args.len() != 2 { return OpResult::Halt; }
                let from = args[0].to_string();
                let to   = args[1].to_string();
                if from.is_empty() { 
                    eprintln!("R307: .replace() called with empty 'from'{}", self.current_span_info(ip)); 
                    return OpResult::Halt; 
                }
                let res = Value::from_string(Arc::new(s.replace(&from, &to)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Slice => {
                if args.len() != 2 { return OpResult::Halt; }
                if !args[0].is_int() || !args[1].is_int() { return OpResult::Halt; }
                let start = args[0].as_i64();
                let end   = args[1].as_i64();
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
                if start < 0 || end > len || start > end {
                    eprintln!("R303: String.slice out of bounds [{}, {}] for len {}{}", start, end, len, self.current_span_info(ip));
                    return OpResult::Halt;
                }
                let res = Value::from_string(Arc::new(chars[start as usize..end as usize].iter().collect::<String>()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Split => {
                if args.is_empty() { return OpResult::Halt; }
                let sep = args[0].to_string();
                let parts: Vec<Value> = s.split(&sep).map(|p| Value::from_string(Arc::new(p.to_string()))).collect();
                let res = Value::from_array(Arc::new(RwLock::new(parts)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::StartsWith => {
                if args.is_empty() { return OpResult::Halt; }
                let prefix = args[0].to_string();
                let res = Value::from_bool(s.starts_with(prefix.as_str()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::EndsWith => {
                if args.is_empty() { return OpResult::Halt; }
                let suffix = args[0].to_string();
                let res = Value::from_bool(s.ends_with(suffix.as_str()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::ToInt => {
                match s.trim().parse::<i64>() {
                    Ok(n) => {
                        let res = Value::from_i64(n);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    }
                    Err(_) => {
                        eprintln!("halt.error: Cannot convert \"{}\" to Integer{}", s, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                }
            }
            MethodKind::ToFloat => {
                match s.trim().parse::<f64>() {
                    Ok(f) => {
                        let res = Value::from_f64(f);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    }
                    Err(_) => {
                        eprintln!("halt.error: Cannot convert \"{}\" to Float{}", s, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                }
            }
            _ => {
                eprintln!("Method {:?} not found on String{}", kind, self.current_span_info(ip));
                return OpResult::Halt;
            }
        }
        OpResult::Continue
    }

    fn handle_map_method(&mut self, dst: u8, map_rc: Arc<RwLock<Vec<(Value, Value)>>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value]) -> OpResult {
        match kind {
            MethodKind::Get => {
                let key = &args[0];
                let map = map_rc.read();
                if let Some((_, v)) = map.iter().find(|(k, _)| k == key) {
                    unsafe { v.inc_ref(); }
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = *v;
                } else {
                    eprintln!("R304: Map key not found: {}{}", key.to_string(), self.current_span_info(ip));
                    return OpResult::Halt;
                }
            }
            MethodKind::Set | MethodKind::Insert => {
                let key = args[0]; 
                let val = args[1];
                let mut map = map_rc.write();
                unsafe { key.inc_ref(); val.inc_ref(); }
                if let Some(e) = map.iter_mut().find(|(k, _)| *k == key) { 
                    let old_k = e.0;
                    let old_v = e.1;
                    e.0 = key;
                    e.1 = val;
                    unsafe { old_k.dec_ref(); old_v.dec_ref(); }
                } else { map.push((key, val)); }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Len | MethodKind::Count | MethodKind::Size => {
                let res = Value::from_i64(map_rc.read().len() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Keys => {
                let mut keys = Vec::new();
                for (k, _) in map_rc.read().iter() { 
                    unsafe { k.inc_ref(); }
                    keys.push(*k);
                }
                let res = Value::from_array(Arc::new(RwLock::new(keys)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Values => {
                let mut vals = Vec::new();
                for (_, v) in map_rc.read().iter() {
                    unsafe { v.inc_ref(); }
                    vals.push(*v);
                }
                let res = Value::from_array(Arc::new(RwLock::new(vals)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Contains => {
                let has = map_rc.read().iter().any(|(k, _)| k == &args[0]);
                let res = Value::from_bool(has);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Remove | MethodKind::Delete => {
                let key = &args[0];
                let mut map = map_rc.write();
                let before = map.len();
                if let Some(pos) = map.iter().position(|(k, _)| k == key) {
                    let (k, v) = map.remove(pos);
                    unsafe { k.dec_ref(); v.dec_ref(); }
                }
                let res = Value::from_bool(map.len() < before);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Clear => { 
                let mut map = map_rc.write();
                for (k, v) in map.drain(..) { unsafe { k.dec_ref(); v.dec_ref(); } }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Show  => { 
                let map_val = Value::from_map(map_rc.clone());
                println!("{}", map_val.to_string()); 
                unsafe { map_val.dec_ref(); }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => { 
                eprintln!("Method {:?} not supported for Map{}", kind, self.current_span_info(ip)); 
                return OpResult::Halt; 
            }
        }
        OpResult::Continue
    }

    fn handle_table_method(&mut self, dst: u8, t_rc: Arc<RwLock<TableData>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value]) -> OpResult {
        let t = t_rc.read();
        match kind {
            MethodKind::Count | MethodKind::Len | MethodKind::Size => {
                let res = Value::from_i64(t.rows.len() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Show => {
                let t_val = Value::from_table(t_rc.clone());
                println!("{}", t_val.to_string());
                unsafe { t_val.dec_ref(); }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Insert | MethodKind::Add => {
                drop(t);
                let mut t_mut = t_rc.write();
                let mut row = Vec::new();
                let mut ai = 0usize;
                let cols = t_mut.columns.clone();
                for col in &cols {
                    if col.is_auto {
                        let cidx = cols.iter().position(|c| c.name == col.name).unwrap();
                        let max = t_mut.rows.iter()
                            .filter_map(|r| if r[cidx].is_int() { Some(r[cidx].as_i64()) } else { None })
                            .max().unwrap_or(0);
                        row.push(Value::from_i64(max + 1));
                    } else {
                        let val = args.get(ai).cloned().unwrap_or(Value::from_bool(false));
                        unsafe { val.inc_ref(); }
                        row.push(val);
                        ai += 1;
                    }
                }
                t_mut.rows.push(row);
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Update => {
                let idx = if args[0].is_int() { args[0].as_i64() } else { -1 };
                let vals = &args[1];
                drop(t);
                if idx >= 0 {
                    let mut t_mut = t_rc.write();
                    if (idx as usize) < t_mut.rows.len() {
                        if vals.is_ptr() && (vals.0 & 0x000F_0000_0000_0000) == TAG_ARR {
                            let arr_rc = vals.as_array();
                            let arr = arr_rc.read();
                            let mut ai = 0usize;
                            for ci in 0..t_mut.columns.len() {
                                if !t_mut.columns[ci].is_auto {
                                    if ai < arr.len() {
                                        let val = arr[ai];
                                        unsafe { val.inc_ref(); }
                                        let old = t_mut.rows[idx as usize][ci];
                                        t_mut.rows[idx as usize][ci] = val;
                                        unsafe { old.dec_ref(); }
                                        ai += 1;
                                    }
                                }
                            }
                            let res = Value::from_bool(true);
                            unsafe { locals[dst as usize].dec_ref(); }
                            locals[dst as usize] = res;
                        } else { 
                            unsafe { locals[dst as usize].dec_ref(); }
                            locals[dst as usize] = Value::from_bool(false);
                        }
                    } else { 
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = Value::from_bool(false);
                    }
                } else { 
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Delete => {
                let idx = if args[0].is_int() { args[0].as_i64() } else { -1 };
                drop(t);
                if idx >= 0 {
                    let mut t_mut = t_rc.write();
                    if (idx as usize) < t_mut.rows.len() {
                        let row = t_mut.rows.remove(idx as usize);
                        for v in row { unsafe { v.dec_ref(); } }
                        let res = Value::from_bool(true);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    } else { 
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = Value::from_bool(false);
                    }
                } else { 
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Where => {
                if !args[0].is_ptr() || (args[0].0 & 0x000F_0000_0000_0000) != TAG_FUNC {
                    eprintln!("R301: Table.where() requires a function. Got: {:x}{}", args[0].0, self.current_span_info(ip));
                    return OpResult::Halt;
                }
                let filter_func = args[0].as_function();
                let row_count = t.rows.len();
                drop(t);
                let mut filtered = Vec::new();
                for i in 0..row_count {
                    let row_ref = Arc::new(RowRef { table: t_rc.clone(), row_idx: i as u32 });
                    let row_val = Value::from_row(row_ref);
                    let mut run_args = vec![row_val];
                    for a in &args[1..] { unsafe { a.inc_ref(); } run_args.push(*a); }
                    if let Some(res) = self.run_frame(filter_func as usize, &run_args) {
                        if res.is_bool() && res.as_bool() {
                            let mut row_copy = Vec::new();
                            for v in &t_rc.read().rows[i] { unsafe { v.inc_ref(); } row_copy.push(*v); }
                            filtered.push(row_copy);
                        }
                        unsafe { res.dec_ref(); }
                    }
                    unsafe { row_val.dec_ref(); }
                    for a in run_args.into_iter().skip(1) { unsafe { a.dec_ref(); } }
                }
                let res = Value::from_table(Arc::new(RwLock::new(
                    TableData { columns: t_rc.read().columns.clone(), rows: filtered }
                )));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Get => {
                let idx = if args[0].is_int() { args[0].as_i64() } else { -1 };
                if idx >= 0 && (idx as usize) < t.rows.len() {
                    let res = Value::from_row(Arc::new(RowRef { table: t_rc.clone(), row_idx: idx as u32 }));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                } else {
                    eprintln!("R303: Table.get index out of bounds: {}{}", idx, self.current_span_info(ip));
                    return OpResult::Halt;
                }
            }
            MethodKind::Join => {
                if args.is_empty() { eprintln!("join: missing arguments{}", self.current_span_info(ip)); return OpResult::Halt; }
                let right_rc = if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_TBL {
                    args[0].as_table()
                } else {
                    eprintln!("join: first argument must be a table{}", self.current_span_info(ip));
                    return OpResult::Halt;
                };
                let pred = if args.len() >= 3 {
                    if args[1].is_ptr() && (args[1].0 & 0x000F_0000_0000_0000) == TAG_STR &&
                       args[2].is_ptr() && (args[2].0 & 0x000F_0000_0000_0000) == TAG_STR {
                        JoinPred::Keys(args[1].as_string().to_string(), args[2].as_string().to_string())
                    } else {
                        eprintln!("join: key args must be strings{}", self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else if args.len() == 2 {
                    if args[1].is_ptr() && (args[1].0 & 0x000F_0000_0000_0000) == TAG_FUNC {
                        JoinPred::Lambda(args[1].as_function() as usize)
                    } else {
                        eprintln!("join: second arg must be a function{}", self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else {
                    eprintln!("join: requires 2 or 3 arguments{}", self.current_span_info(ip));
                    return OpResult::Halt;
                };
                let left_data  = t.clone();
                let right_data = right_rc.read().clone();
                drop(t);
                let result = join_tables(&left_data, &right_data, &pred, "b", self);
                let res = Value::from_table(Arc::new(RwLock::new(result)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Clear => {
                drop(t);
                let mut t_mut = t_rc.write();
                for row in t_mut.rows.drain(..) { for v in row { unsafe { v.dec_ref(); } } }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => { eprintln!("Method {:?} not supported for Table{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_row_method(&mut self, dst: u8, row_ref: Arc<RowRef>, kind: MethodKind, ip: usize, locals: &mut [Value]) -> OpResult {
        match kind {
            MethodKind::Show => {
                let t = row_ref.table.read();
                println!("{:?}", t.rows[row_ref.row_idx as usize]);
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => {
                eprintln!("Method {:?} not supported for Row{}", kind, self.current_span_info(ip));
                return OpResult::Halt;
            }
        }
        OpResult::Continue
    }

    fn handle_row_custom(&mut self, dst: u8, row_ref: Arc<RowRef>, method_name: &str, ip: usize, locals: &mut [Value]) -> OpResult {
        let t = row_ref.table.read();
        if let Some(col_idx) = t.columns.iter().position(|c| c.name == method_name) {
            let v = t.rows[row_ref.row_idx as usize][col_idx];
            unsafe { v.inc_ref(); }
            unsafe { locals[dst as usize].dec_ref(); }
            locals[dst as usize] = v;
        } else {
            match method_name {
                "show" => {
                    let row_val = Value::from_row(row_ref.clone());
                    println!("{}", row_val.to_string());
                    unsafe { row_val.dec_ref(); }
                    let res = Value::from_bool(true);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                _ => { eprintln!("Unknown Row member: {}{}", method_name, self.current_span_info(ip)); return OpResult::Halt; }
            }
        }
        OpResult::Continue
    }

    fn handle_date_method(&mut self, dst: u8, d: chrono::NaiveDateTime, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value]) -> OpResult {
        use chrono::Datelike;
        use chrono::Timelike;
        match kind {
            MethodKind::Year   => {
                let res = Value::from_i64(d.year() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Month  => {
                let res = Value::from_i64(d.month() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Day    => {
                let res = Value::from_i64(d.day() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Hour   => {
                let res = Value::from_i64(d.hour() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Minute => {
                let res = Value::from_i64(d.minute() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Second => {
                let res = Value::from_i64(d.second() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Format => {
                let fmt_str = if let Some(v) = args.first() {
                    if v.is_ptr() && (v.0 & 0x000F_0000_0000_0000) == TAG_STR {
                        v.as_string().replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d")
                            .replace("HH", "%H").replace("mm", "%M").replace("ss", "%S")
                            .replace("SSS", "%3f").replace("ms", "%3f")
                    } else { "%Y-%m-%d %H:%M:%S".to_string() }
                } else {
                    "%Y-%m-%d %H:%M:%S".to_string()
                };
                let res = Value::from_string(Arc::new(d.format(&fmt_str).to_string()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => { eprintln!("Method {:?} not supported for Date{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_json_method(&mut self, dst: u8, j_rc: Arc<RwLock<serde_json::Value>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value]) -> OpResult {
        let mut j_mut = j_rc.write();
        match kind {
            MethodKind::Set | MethodKind::Insert => {
                if args.len() >= 2 {
                    if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_STR {
                        let path = args[0].as_string();
                        let val = value_to_json(&args[1]);
                        set_json_value_at_path(&mut j_mut, &path, val);
                    }
                }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Push | MethodKind::Append => {
                if args.len() >= 2 {
                    if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_STR {
                        let path = args[0].as_string();
                        let val = value_to_json(&args[1]);
                        let pp = normalize_json_path(&path);
                        if let Some(target) = j_mut.pointer_mut(&pp) {
                            if let Some(arr) = target.as_array_mut() {
                                arr.push(val);
                            }
                        }
                    }
                }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Count | MethodKind::Len | MethodKind::Size => {
                let n = j_mut.as_array().map(|a| a.len())
                    .or_else(|| j_mut.as_object().map(|o| o.len()))
                    .unwrap_or(0);
                let res = Value::from_i64(n as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Exists => {
                let found = if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_STR {
                    let path = args[0].as_string();
                    let pp = normalize_json_path(&path);
                    j_mut.pointer(&pp).map(|v| !v.is_null()).unwrap_or(false)
                } else { false };
                let res = Value::from_bool(found);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Get => {
                let path_storage;
                let path = if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_STR {
                    let s = args[0].as_string();
                    (*s).clone()
                } else if args[0].is_int() {
                    path_storage = format!("/{}", args[0].as_i64());
                    path_storage.clone()
                } else {
                    "".to_string()
                };
                let pp = normalize_json_path(&path);
                let res = if let Some(v) = j_mut.pointer(&pp) {
                    json_serde_to_value(v)
                } else { Value::from_bool(false) };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Inject => {
                let ok = if args.len() == 2 {
                    if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_MAP &&
                       args[1].is_ptr() && (args[1].0 & 0x000F_0000_0000_0000) == TAG_TBL {
                        inject_json_into_table(&mut args[1].as_table().write(), &j_mut, &args[0].as_map().read());
                        true
                    } else { false }
                } else if args.len() == 3 {
                    if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_STR &&
                       args[1].is_ptr() && (args[1].0 & 0x000F_0000_0000_0000) == TAG_MAP &&
                       args[2].is_ptr() && (args[2].0 & 0x000F_0000_0000_0000) == TAG_TBL {
                        let key = args[0].as_string();
                        let pp = normalize_json_path(&key);
                        let sub_json = j_mut.pointer(&pp).unwrap_or(&serde_json::Value::Null);
                        inject_json_into_table(&mut args[2].as_table().write(), sub_json, &args[1].as_map().read());
                        true
                    } else { false }
                } else { false };
                let res = Value::from_bool(ok);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::ToStr => {
                let res = Value::from_string(Arc::new(j_mut.to_string()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => {
                eprintln!("Method {:?} not supported for JSON{}", kind, self.current_span_info(ip));
                return OpResult::Halt;
            }
        }
        OpResult::Continue
    }

    fn handle_json_custom(&mut self, dst: u8, j_rc: Arc<RwLock<serde_json::Value>>, field_name: &str, _args: &[Value], _ip: usize, locals: &mut [Value]) -> OpResult {
        let j = j_rc.read();
        let pp = normalize_json_path(field_name);
        let res = if let Some(v) = j.pointer(&pp) {
            json_serde_to_value(v)
        } else {
            Value::from_bool(false)
        };
        unsafe { locals[dst as usize].dec_ref(); }
        locals[dst as usize] = res;
        OpResult::Continue
    }

    fn handle_fiber_method(&mut self, dst: u8, fiber_rc: Arc<RwLock<FiberState>>, kind: MethodKind, ip: usize, locals: &mut [Value]) -> OpResult {
        match kind {
            MethodKind::Next => {
                let cached = fiber_rc.write().yielded_value.take();
                let res = if let Some(val) = cached {
                    val
                } else if fiber_rc.read().is_done {
                    Value::from_bool(false)
                } else {
                    self.resume_fiber(fiber_rc.clone(), true).unwrap_or(Value::from_bool(false))
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Run => {
                if !fiber_rc.read().is_done {
                    let cached = fiber_rc.write().yielded_value.take();
                    if cached.is_none() { self.resume_fiber(fiber_rc, false); }
                }
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::IsDone => {
                let f = fiber_rc.read();
                let res_bool = if f.yielded_value.is_some() {
                    false
                } else {
                    f.is_done
                };

                let res = Value::from_bool(res_bool);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Close => {
                fiber_rc.write().is_done = true;
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => { eprintln!("Method {:?} not supported for Fiber{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

}

fn build_response_json(result: Result<ureq::Response, ureq::Error>) -> serde_json::Value {
    match result {
        Ok(resp) => {
            let status = resp.status();
            let mut h_map = serde_json::Map::new();
            for name in resp.headers_names() {
                if let Some(val) = resp.header(&name) {
                    h_map.insert(name, serde_json::Value::String(val.to_string()));
                }
            }
            let text = resp.into_string().unwrap_or_default();
            if text.len() > 10 * 1024 * 1024 {
                let mut res = serde_json::Map::new();
                res.insert("status".to_string(), serde_json::Value::Number(413.into()));
                res.insert("ok".to_string(),     serde_json::Value::Bool(false));
                res.insert("error".to_string(),  serde_json::Value::String("Body too large".to_string()));
                serde_json::Value::Object(res)
            } else {
                let body_val = serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));
                let mut res = serde_json::Map::new();
                res.insert("status".to_string(),  serde_json::Value::Number(status.into()));
                res.insert("ok".to_string(),      serde_json::Value::Bool(status >= 200 && status < 300));
                res.insert("body".to_string(),    body_val);
                res.insert("headers".to_string(), serde_json::Value::Object(h_map));
                serde_json::Value::Object(res)
            }
        }
        Err(e) => {
            let mut res = serde_json::Map::new();
            res.insert("status".to_string(), serde_json::Value::Number(0.into()));
            res.insert("ok".to_string(),     serde_json::Value::Bool(false));
            res.insert("error".to_string(),  serde_json::Value::String(e.to_string()));
            serde_json::Value::Object(res)
        }
    }
}

fn json_serde_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null    => Value::from_bool(false),
        serde_json::Value::Bool(b) => Value::from_bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { Value::from_i64(i) }
            else if let Some(f) = n.as_f64() { Value::from_f64(f) }
            else { Value::from_i64(0) }
        }
        serde_json::Value::String(s) => Value::from_string(Arc::new(s.clone())),
        serde_json::Value::Array(arr) => {
            let elements: Vec<Value> = arr.iter().map(|e| json_serde_to_value(e)).collect();
            Value::from_array(Arc::new(RwLock::new(elements)))
        }
        serde_json::Value::Object(_) => {
            Value::from_json(Arc::new(RwLock::new(v.clone())))
        }
    }
}

pub fn normalize_json_path(path: &str) -> String {
    if path.is_empty() { return String::new(); }
    let mut p = path.replace('.', "/").replace('[', "/").replace(']', "");
    if !p.starts_with('/') { p.insert(0, '/'); }
    p
}

fn set_json_value_at_path(target: &mut serde_json::Value, path: &str, value: serde_json::Value) {
    let pointer = normalize_json_path(path);
    let parts: Vec<&str> = pointer.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        *target = value;
        return;
    }
    let mut current = target;
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        if let Ok(idx) = part.parse::<usize>() {
            if !current.is_array() {
                *current = serde_json::Value::Array(Vec::new());
            }
            let arr = current.as_array_mut().unwrap();
            while arr.len() <= idx {
                arr.push(serde_json::Value::Null);
            }
            if is_last {
                arr[idx] = value;
                return;
            }
            current = &mut arr[idx];
        } else {
            if !current.is_object() {
                *current = serde_json::Value::Object(serde_json::Map::new());
            }
            let obj = current.as_object_mut().unwrap();
            if is_last {
                obj.insert(part.to_string(), value);
                return;
            }
            let next_is_array = if i + 1 < parts.len() {
                parts[i+1].parse::<usize>().is_ok()
            } else {
                false
            };
            current = obj.entry(part.to_string()).or_insert_with(|| {
                if next_is_array {
                    serde_json::Value::Array(Vec::new())
                } else {
                    serde_json::Value::Object(serde_json::Map::new())
                }
            });
        }
    }
}

pub fn value_to_json(v: &Value) -> serde_json::Value {
    if v.is_int() { return serde_json::Value::Number(v.as_i64().into()); }
    if v.is_float() { return serde_json::Number::from_f64(v.as_f64()).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null); }
    if v.is_bool() { return serde_json::Value::Bool(v.as_bool()); }
    if !v.is_ptr() { return serde_json::Value::Null; }
    
    let tag = v.0 & 0x000F_0000_0000_0000;
    match tag {
        TAG_STR => serde_json::Value::String(v.as_string().to_string()),
        TAG_ARR => {
            let a_rc = v.as_array();
            let a = a_rc.read();
            serde_json::Value::Array(a.iter().map(value_to_json).collect())
        }
        TAG_MAP => {
            let b_rc = v.as_map();
            let b = b_rc.read();
            let mut obj = serde_json::Map::new();
            for (k, val) in b.iter() { obj.insert(k.to_string(), value_to_json(val)); }
            serde_json::Value::Object(obj)
        }
        TAG_JSON => v.as_json().read().clone(),
        TAG_DATE => {
            let ts = v.as_date();
            let dt = chrono::DateTime::from_timestamp(ts, 0).unwrap().with_timezone(&chrono::Local).naive_local();
            serde_json::Value::String(dt.format("%Y-%m-%d").to_string())
        },
        _ => serde_json::Value::Null,
    }
}



pub fn is_safe_url(url_str: &str) -> Result<(), String> {
    if url_str.starts_with("file://") {
        return Err("HALT.FATAL: SSRF - file:// URLs are forbidden".to_string());
    }
    let host = if let Some(start) = url_str.find("://") {
        let remainder = &url_str[start+3..];
        let end = remainder.find('/').unwrap_or(remainder.len());
        let mut host_port = &remainder[..end];
        if let Some(p) = host_port.find('@') { host_port = &host_port[p+1..]; }
        if let Some(p) = host_port.find(':') { host_port = &host_port[..p]; }
        host_port.to_lowercase()
    } else {
        url_str.to_lowercase()
    };
    if host == "169.254.169.254" || host.starts_with("169.254.") {
        return Err("HALT.FATAL: SSRF - Link-local addresses are forbidden".to_string());
    }
    let is_localhost = host == "localhost" || host == "127.0.0.1" || host == "::1";
    if !is_localhost {
        if host.starts_with("10.") ||
            host.starts_with("192.168.") ||
            host.starts_with("172.16.") || host.starts_with("172.17.") ||
            host.starts_with("172.18.") || host.starts_with("172.19.") ||
            host.starts_with("172.20.") || host.starts_with("172.21.") ||
            host.starts_with("172.22.") || host.starts_with("172.23.") ||
            host.starts_with("172.24.") || host.starts_with("172.25.") ||
            host.starts_with("172.26.") || host.starts_with("172.27.") ||
            host.starts_with("172.28.") || host.starts_with("172.29.") ||
            host.starts_with("172.30.") || host.starts_with("172.31.") {
            return Err("HALT.ERROR: SSRF - Private IP ranges are blocked in production".to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_value_size() {
        println!("Value size: {}", std::mem::size_of::<Value>());
        assert!(std::mem::size_of::<Value>() <= 24);
    }   
}            