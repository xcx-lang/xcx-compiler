use std::sync::Arc;
use parking_lot::{RwLock, RwLockWriteGuard, Mutex};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering, AtomicPtr};
pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);
use argon2::password_hash::{PasswordVerifier, PasswordHash};
use base64::Engine;
use std::io::Write;
use rand::Rng;
use crossterm::{
    execute,
    terminal::{Clear, ClearType, enable_raw_mode, disable_raw_mode},
    cursor::{MoveTo, Show, Hide},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Push, Pop, Len, Count, Size, IsEmpty, Clear, Contains, Get, Insert, Update, Delete, Find, Join, Show, Sort, Reverse,
    Add, Remove, Has, Length, Upper, Lower, Trim, IndexOf, LastIndexOf, Replace, Slice, Split, StartsWith, EndsWith,
    ToInt, ToFloat, Set, Keys, Values, Where, Year, Month, Day, Hour, Minute, Second, Format, Exists, Append, Inject, ToStr, ToJson,
    Next, Run, IsDone, Close, Begin, Commit, Rollback, Query, QueryRaw, Sync, Drop, Fetch, Save, Truncate, Exec, IsOpen, First,
    Key, Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTag {
    Int,
    Float,
    String,
    Bool,
    Unknown,
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
    Input { dst: u8, ty: TypeTag },
    HaltAlert { src: u8 }, 
    HaltError { src: u8 }, 
    HaltFatal { src: u8 },
    TerminalExit,
    TerminalRun { dst: u8, cmd_src: u8 },

    TerminalClear,
    TerminalRaw,
    TerminalNormal,
    TerminalCursor { on: bool },
    TerminalMove { x_src: u8, y_src: u8 },
    TerminalWrite { src: u8 },

    InputKey { dst: u8 },
    InputKeyWait { dst: u8 },
    InputReady { dst: u8 },

    
    Call { dst: u8, func_idx: u32, base: u8, arg_count: u8 },
    Return { src: u8 },
    ReturnVoid,
    Halt,

    SetName { src: u8, name_idx: u32 },

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
    RandomInt { dst: u8, min: u8, max: u8, step: u8, has_step: u8 },
    RandomFloat { dst: u8, min: u8, max: u8, step: u8, has_step: u8 },

    // Store operations
    StoreWrite { base: u8 }, 
    StoreRead { dst: u8, base: u8 },
    StoreAppend { base: u8 },
    StoreExists { dst: u8, base: u8 },
    StoreDelete { base: u8 },
    StoreList { dst: u8, base: u8 },
    StoreIsDir { dst: u8, base: u8 },
    StoreSize { dst: u8, base: u8 },
    StoreMkdir { base: u8 },
    StoreGlob { dst: u8, base: u8 },
    StoreZip { dst: u8, base: u8 },
    StoreUnzip { dst: u8, base: u8 },

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
    ArrayLoopNext { idx_reg: u8, size_reg: u8, target: u32 },
    DatabaseInit { dst: u8, engine_src: u8, path_src: u8, tables_base_reg: u8, table_count: u32 },
    MethodCallNamed { dst: u8, kind: MethodKind, base: u8, arg_count: u8, names_idx: u32 },
}

fn map_key_code_to_value(code: KeyCode) -> Value {
    match code {
        KeyCode::Char(c) => Value::from_string(Arc::new(vec![c as u8])),
        KeyCode::Esc => Value::from_string(Arc::new(b"ESC".to_vec())),
        KeyCode::Enter => Value::from_string(Arc::new(b"ENTER".to_vec())),
        KeyCode::Tab => Value::from_string(Arc::new(b"TAB".to_vec())),
        KeyCode::Backspace => Value::from_string(Arc::new(b"BACKSPACE".to_vec())),
        KeyCode::Up => Value::from_string(Arc::new(b"UP".to_vec())),
        KeyCode::Down => Value::from_string(Arc::new(b"DOWN".to_vec())),
        KeyCode::Left => Value::from_string(Arc::new(b"LEFT".to_vec())),
        KeyCode::Right => Value::from_string(Arc::new(b"RIGHT".to_vec())),
        KeyCode::F(n) => Value::from_string(Arc::new(format!("F{}", n).into_bytes())),
        _ => Value::from_bool(false),
    }
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

    // Random ops
    RandomInt { dst: u8, min: u8, max: u8, step: u8, has_step: u8 },
    RandomFloat { dst: u8, min: u8, max: u8, step: u8, has_step: u8, step_is_float: bool },

    PowInt   { dst: u8, src1: u8, src2: u8 },
    PowFloat { dst: u8, src1: u8, src2: u8 },
    IntConcat { dst: u8, src1: u8, src2: u8 },
    
    Has          { dst: u8, src1: u8, src2: u8 },
    RandomChoice { dst: u8, src: u8 },

    ArraySize { dst: u8, src: u8 },
    ArrayGet  { dst: u8, arr_reg: u8, idx_reg: u8, fail_ip: usize },
    ArrayPush { arr_reg: u8, val_reg: u8 },

    SetSize     { dst: u8, src: u8 },
    SetContains { dst: u8, set_reg: u8, val_reg: u8 },
    ArrayUpdate { arr_reg: u8, idx_reg: u8, val_reg: u8, fail_ip: usize },
}

#[derive(Debug)]
pub struct Trace {
    pub ops: Vec<TraceOp>,
    pub start_ip: usize,
    pub native_ptr: std::sync::atomic::AtomicPtr<u8>,
    pub min_locals: usize,
}

pub struct DatabaseData {
    pub conn: Arc<Mutex<rusqlite::Connection>>,
    pub engine: String,
    pub path: String,
    pub tables: Arc<RwLock<HashMap<String, Value>>>,
}

#[derive(Clone)]
pub struct SqlBinding {
    pub db_conn: Arc<Mutex<rusqlite::Connection>>,
    pub table_name: String,
}

impl std::fmt::Debug for SqlBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlBinding")
            .field("table_name", &self.table_name)
            .finish_non_exhaustive()
    }
}

impl Drop for DatabaseData {
    fn drop(&mut self) {
        let tables = self.tables.read();
        for (_, val) in tables.iter() {
            unsafe { val.dec_ref(); }
        }
    }
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

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Value(pub u64);

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        if self.is_float() && other.is_float() {
            return self.as_f64() == other.as_f64();
        }
        if self.is_string() && other.is_string() {
            let s1 = self.as_string();
            let s2 = other.as_string();
            return *s1 == *s2;
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
pub const TAG_DB:    u64 = 0x000D_0000_0000_0000;

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
    #[inline] pub fn is_set(&self)    -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_SET) }
    #[inline] pub fn is_map(&self)    -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_MAP) }
    #[inline] pub fn is_db(&self)     -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_DB) }
    #[inline] pub fn is_fiber(&self)  -> bool { (self.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_FIB) }
    #[inline] pub fn is_bool_false(&self) -> bool { self.is_bool() && !self.as_bool() }

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
            TAG_STR  => { unsafe { Arc::increment_strong_count(p as *const Vec<u8>); } }
            TAG_ARR  => { unsafe { Arc::increment_strong_count(p as *const RwLock<Vec<Value>>); } }
            TAG_SET  => { unsafe { Arc::increment_strong_count(p as *const RwLock<SetData>); } }
            TAG_MAP  => { unsafe { Arc::increment_strong_count(p as *const RwLock<Vec<(Value, Value)>>); } }
            TAG_TBL  => { unsafe { Arc::increment_strong_count(p as *const RwLock<TableData>); } }
            TAG_JSON => { unsafe { Arc::increment_strong_count(p as *const RwLock<serde_json::Value>); } }
            TAG_FIB  => { unsafe { Arc::increment_strong_count(p as *const RwLock<FiberState>); } }
            TAG_ROW  => { unsafe { Arc::increment_strong_count(p as *const RowRef); } }
            TAG_DB   => { unsafe { Arc::increment_strong_count(p as *const DatabaseData); } }
            _ => {}
        }
    }

    #[inline]
    pub unsafe fn dec_ref(&self) {
        if !self.is_ptr() { return; }
        let tag = self.0 & 0x000F_0000_0000_0000;
        let p = self.unpack_ptr::<()>();
        match tag {
            TAG_STR  => { unsafe { Arc::decrement_strong_count(p as *const Vec<u8>); } }
            TAG_ARR  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<Vec<Value>>); } }
            TAG_SET  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<SetData>); } }
            TAG_MAP  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<Vec<(Value, Value)>>); } }
            TAG_TBL  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<TableData>); } }
            TAG_JSON => { unsafe { Arc::decrement_strong_count(p as *const RwLock<serde_json::Value>); } }
            TAG_FIB  => { unsafe { Arc::decrement_strong_count(p as *const RwLock<FiberState>); } }
            TAG_ROW  => { unsafe { Arc::decrement_strong_count(p as *const RowRef); } }
            TAG_DB   => { unsafe { Arc::decrement_strong_count(p as *const DatabaseData); } }
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
                TAG_DB   => 13,
                _ => 255,
            }
        } else { 255 }
    }

    #[inline] pub fn from_string(s: Arc<Vec<u8>>) -> Self { Self::pack_ptr(Arc::into_raw(s), TAG_STR) }
    pub fn from_string_array(strs: Arc<Vec<String>>) -> Self {
        let mut vals = Vec::with_capacity(strs.len());
        for s in strs.iter() {
            vals.push(Value::from_string(Arc::new(s.as_bytes().to_vec())));
        }
        Value::from_array(Arc::new(RwLock::new(vals)))
    }
    #[inline] pub fn from_array(a: Arc<RwLock<Vec<Value>>>) -> Self { Self::pack_ptr(Arc::into_raw(a), TAG_ARR) }
    #[inline] pub fn from_set(s: Arc<RwLock<SetData>>) -> Self { Self::pack_ptr(Arc::into_raw(s), TAG_SET) }
    #[inline] pub fn from_map(m: Arc<RwLock<Vec<(Value, Value)>>>) -> Self { Self::pack_ptr(Arc::into_raw(m), TAG_MAP) }
    #[inline] pub fn from_table(t: Arc<RwLock<TableData>>) -> Self { Self::pack_ptr(Arc::into_raw(t), TAG_TBL) }
    #[inline] pub fn from_json(j: Arc<RwLock<serde_json::Value>>) -> Self { Self::pack_ptr(Arc::into_raw(j), TAG_JSON) }
    #[inline] pub fn from_fiber(f: Arc<RwLock<FiberState>>) -> Self { Self::pack_ptr(Arc::into_raw(f), TAG_FIB) }
    #[inline] pub fn from_db(d: Arc<DatabaseData>) -> Self { Self::pack_ptr(Arc::into_raw(d), TAG_DB) }
    #[inline] pub fn from_date(ts: i64) -> Self { Self(QNAN_BASE | TAG_DATE | (ts as u64 & 0x0000_FFFF_FFFF_FFFF)) }
    #[inline] pub fn from_function(id: u32) -> Self { Self(QNAN_BASE | TAG_FUNC | (id as u64)) }
    #[inline] pub fn from_row(r: Arc<RowRef>) -> Self { Self::pack_ptr(Arc::into_raw(r), TAG_ROW) }

    pub fn as_string(&self) -> Arc<Vec<u8>> { unsafe { let p = self.unpack_ptr::<Vec<u8>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_array(&self) -> Arc<RwLock<Vec<Value>>> { unsafe { let p = self.unpack_ptr::<RwLock<Vec<Value>>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_set(&self) -> Arc<RwLock<SetData>> { unsafe { let p = self.unpack_ptr::<RwLock<SetData>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_map(&self) -> Arc<RwLock<Vec<(Value, Value)>>> { unsafe { let p = self.unpack_ptr::<RwLock<Vec<(Value, Value)>>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_table(&self) -> Arc<RwLock<TableData>> { unsafe { let p = self.unpack_ptr::<RwLock<TableData>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_json(&self) -> Arc<RwLock<serde_json::Value>> { unsafe { let p = self.unpack_ptr::<RwLock<serde_json::Value>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_fiber(&self) -> Arc<RwLock<FiberState>> { unsafe { let p = self.unpack_ptr::<RwLock<FiberState>>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_row(&self) -> Arc<RowRef> { unsafe { let p = self.unpack_ptr::<RowRef>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    pub fn as_db(&self) -> Arc<DatabaseData> { unsafe { let p = self.unpack_ptr::<DatabaseData>(); let arc = Arc::from_raw(p); let cl = arc.clone(); std::mem::forget(arc); cl } }
    #[inline] pub fn as_date(&self) -> i64 { (self.0 & 0x0000_FFFF_FFFF_FFFF) as i64 }
    #[inline] pub fn as_function(&self) -> u32 { (self.0 & 0x0000_FFFF_FFFF_FFFF) as u32 }
    
    pub fn as_array_opt(&self) -> Option<Arc<RwLock<Vec<Value>>>> {
        if self.is_array() { Some(self.as_array()) } else { None }
    }

    pub fn to_sql_value(&self) -> rusqlite::types::Value {
        if self.is_int() { rusqlite::types::Value::Integer(self.as_i64()) }
        else if self.is_float() { rusqlite::types::Value::Real(self.as_f64()) }
        else if self.is_bool() { rusqlite::types::Value::Integer(if self.as_bool() { 1 } else { 0 }) }
        else if self.is_string() { 
            let b = self.as_string();
            rusqlite::types::Value::Text(String::from_utf8_lossy(&b).into_owned())
        } else { rusqlite::types::Value::Null }
    }

    #[inline]
    pub fn matches_str(&self, other: &str) -> bool {
        if !self.is_string() { return false; }
        let b = unsafe { &*(self.unpack_ptr::<Vec<u8>>()) };
        b.as_slice() == other.as_bytes()
    }

    pub fn to_string(&self) -> String {
        if self.is_float() { self.as_f64().to_string() }
        else if self.is_int() { self.as_i64().to_string() }
        else if self.is_bool() { self.as_bool().to_string() }
        else if self.is_ptr() {
            let tag = self.0 & 0x000F_0000_0000_0000;
            match tag {
                TAG_STR => { 
                    let b = unsafe { &*(self.unpack_ptr::<Vec<u8>>()) };
                    String::from_utf8_lossy(b).into_owned()
                }
                TAG_JSON => {
                    let arc = self.as_json();
                    serde_json::to_string_pretty(&*arc.read()).unwrap_or_else(|_| "null".to_string())
                }
                TAG_ARR  => { 
                    serde_json::to_string_pretty(&value_to_json(self)).unwrap_or_else(|_| "[]".to_string())
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
                    serde_json::to_string_pretty(&value_to_json(self)).unwrap_or_else(|_| "{}".to_string())
                }
                TAG_TBL  => {
                    let arc = self.as_table();
                    format!("Table(rows: {})", arc.read().rows.len())
                }
                TAG_FUNC => format!("Function({})", self.as_function()),
                TAG_ROW  => format!("Row({})", self.as_row().row_idx),
                TAG_FIB  => {
                    let arc = self.as_fiber();
                    let fib = arc.read();
                    if fib.is_done { "Fiber(done)".to_string() }
                    else { format!("Fiber(ip={})", fib.ip) }
                }
                TAG_DB => {
                    let arc = self.as_db();
                    format!("Database(engine={}, path={})", arc.engine, arc.path)
                }
                _ => format!("Ptr({:x})", self.0),
            }
        }
        else if self.is_date() {
            let ts = self.as_date(); 
            let dt = chrono::DateTime::from_timestamp_millis(ts).unwrap().naive_utc();
            dt.format("%Y-%m-%d").to_string()
        }
        else { format!("Value({:x})", self.0) }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct VMColumn {
    pub name: String,
    pub ty: crate::parser::ast::Type,
    pub is_auto: bool,
    pub is_pk: bool,
}

#[derive(Debug, Clone)]
pub struct TableData {
    pub table_name: String,
    pub columns: Vec<VMColumn>,
    pub rows: Vec<Vec<Value>>,
    pub sql_binding: Option<SqlBinding>,
    pub sql_where: Option<String>,
    pub pending_op: Option<MethodKind>,
}

impl PartialEq for TableData {
    fn eq(&self, other: &Self) -> bool {
        self.table_name == other.table_name && self.columns == other.columns && self.rows == other.rows && self.sql_where == other.sql_where && self.pending_op == other.pending_op
    }
}

impl Drop for TableData {
    fn drop(&mut self) {
        for row in self.rows.iter() {
            for val in row.iter() {
                unsafe { val.dec_ref(); }
            }
        }
    }
}

impl TableData {
    pub fn to_formatted_grid(&self) -> String {
        if self.columns.is_empty() { return "Empty Table".into(); }
        let mut widths: Vec<usize> = self.columns.iter().map(|c| c.name.len()).collect();
        for row in &self.rows {
            for (i, val) in row.iter().enumerate() {
                if i < widths.len() {
                    let s = val.to_string();
                    if s.len() > widths[i] { widths[i] = s.len(); }
                }
            }
        }
        let mut res = String::new();
        // Header
        res.push('|');
        for (i, col) in self.columns.iter().enumerate() {
            res.push(' ');
            let name = &col.name;
            res.push_str(&format!("{:width$}", name, width = widths[i]));
            res.push_str(" |");
        }
        res.push('\n');
        // Separator
        res.push('|');
        for w in &widths {
            res.push('-');
            for _ in 0..*w { res.push('-'); }
            res.push('-');
            res.push('|');
        }
        res.push('\n');
        // Rows
        for row in &self.rows {
            res.push('|');
            for (i, val) in row.iter().enumerate() {
                if i < widths.len() {
                    res.push(' ');
                    res.push_str(&format!("{:width$}", val.to_string(), width = widths[i]));
                    res.push_str(" |");
                }
            }
            res.push('\n');
        }
        res
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut rows = Vec::with_capacity(self.rows.len());
        for row in &self.rows {
            let mut obj = serde_json::Map::new();
            for (i, col) in self.columns.iter().enumerate() {
                if i < row.len() {
                    obj.insert(col.name.clone(), value_to_json(&row[i]));
                }
            }
            rows.push(serde_json::Value::Object(obj));
        }
        serde_json::Value::Array(rows)
    }
}

#[derive(Debug, Clone)]
pub struct FiberState {
    pub func_id: usize,
    pub ip: usize,
    pub locals: Vec<Value>,
    pub is_done: bool,
    pub yielded_value: Option<Value>,
    pub trace_revision: u64,
}

impl Drop for FiberState {
    fn drop(&mut self) {
        for val in self.locals.iter() {
            unsafe { val.dec_ref(); }
        }
    }
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
                TAG_STR  => write!(f, "{}", String::from_utf8_lossy(&self.as_string())),
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
                TAG_TBL  => {
                    let arc = self.as_table();
                    write!(f, "Table(rows: {})", arc.read().rows.len())
                }
                TAG_FUNC => write!(f, "Function({})", self.as_function()),
                TAG_ROW  => write!(f, "Row({})", self.as_row().row_idx),
                TAG_JSON => {
                    let arc = self.as_json();
                    write!(f, "{}", serde_json::to_string_pretty(&*arc.read()).unwrap_or_else(|_| "null".to_string()))
                }
                TAG_FIB  => {
                    let arc = self.as_fiber();
                    let fib = arc.read();
                    if fib.is_done { write!(f, "Fiber(done)") }
                    else { write!(f, "Fiber(ip={})", fib.ip) }
                }
                _ => write!(f, "Ptr({:x})", self.0),
            }
        } else if self.is_date() {
            let ts = self.as_date();
            let dt = chrono::DateTime::from_timestamp_millis(ts).unwrap().naive_utc();
            write!(f, "{}", dt.format("%Y-%m-%d"))
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
    pub has_loops: bool,
    /// Functions that contain terminal/input opcodes cannot be JIT-compiled
    /// because the JIT has no support for those opcodes.
    pub has_terminal_ops: bool,
    pub jit_ptr: Arc<std::sync::atomic::AtomicPtr<u8>>,
    pub call_count: Arc<std::sync::atomic::AtomicUsize>,
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
    pub jit_revision: std::sync::atomic::AtomicU64,
}

#[derive(Debug, Clone)]
enum OpResult {
    Continue,
    Return(Option<Value>),
    Yield(Option<Value>),
    Call(Arc<RwLock<FiberState>>, Option<Value>, u8), // target fiber, arg, dst register
    Halt,
}



impl VM {
    pub fn new() -> Self {
        Self {
            globals: Arc::new(RwLock::new(vec![Value::from_bool(false); 1024])),
            error_count: std::sync::atomic::AtomicUsize::new(0),
            traces: Arc::new(RwLock::new(std::collections::HashMap::new())),
            jit: parking_lot::Mutex::new(crate::backend::jit::JIT::new()),
            jit_revision: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl Drop for VM {
    fn drop(&mut self) {
        let mut globals = self.globals.write();
        for val in globals.iter() {
            unsafe { val.dec_ref(); }
        }
        globals.clear();
    }
}

impl VM {

    #[allow(dead_code)]
    pub fn get_global(&self, idx: usize) -> Option<Value> {
        self.globals.read().get(idx).cloned()
    }

    pub fn run(self: Arc<Self>, main_chunk: FunctionChunk, ctx: SharedContext) {
        // Bytecode dumping disabled for production.
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
            terminal_raw_enabled: false,
            hot_counts_pool: Vec::with_capacity(32),
            trace_cache_pool: Vec::with_capacity(32),
            locals_pool: Vec::with_capacity(32),
            trace_revision: 0,
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

pub struct Executor {
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
    terminal_raw_enabled: bool,
    // [XCX 3.0 Fast-Path Pooling]
    // We pool vectors to avoid O(N) allocations in recursive calls.
    hot_counts_pool: Vec<Vec<usize>>,
    trace_cache_pool: Vec<Vec<Option<Arc<Trace>>>>,
    locals_pool: Vec<Vec<Value>>,
    trace_revision: u64,
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
                    if &**col_match == col.name.as_bytes() {
                        let pointer = normalize_json_path(&String::from_utf8_lossy(json_path.as_ref()));
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
    Closure(usize, Vec<Value>),
}

fn join_tables<'a>(
    left: &TableData,
    right: &TableData,
    pred: &JoinPred,
    right_name: &str,
    executor: &mut Executor,
    vm_arc: &'a Arc<VM>,
    glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>,
) -> TableData {
    let right_key_name: Option<&str> = match pred {
        JoinPred::Keys(_, rk) => Some(rk.as_str()),
        JoinPred::Lambda(_) => None,
        JoinPred::Closure(_, _) => None,
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
        out_cols.push(VMColumn { name: out_name, ty: col.ty.clone(), is_auto: col.is_auto, is_pk: col.is_pk });
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
                    let m = matches!(executor.run_frame_with_guard(executor.ctx.functions[*fid].clone(), &[row_a, row_b], vm_arc, glbs, *fid), Some(res) if res.is_bool() && res.as_bool());
                    unsafe { row_a.dec_ref(); row_b.dec_ref(); }
                    m
                }
                JoinPred::Closure(fid, captures) => {
                    let row_a = Value::from_row(Arc::new(RowRef { table: left_rc.clone(), row_idx: li as u32 }));
                    let row_b = Value::from_row(Arc::new(RowRef { table: right_rc.clone(), row_idx: ri as u32 }));
                    let mut run_args = vec![row_a, row_b];
                    for v in captures { unsafe { v.inc_ref(); } run_args.push(*v); }
                    let m = matches!(executor.run_frame_with_guard(executor.ctx.functions[*fid].clone(), &run_args, vm_arc, glbs, *fid), Some(res) if res.is_bool() && res.as_bool());
                    for v in run_args { unsafe { v.dec_ref(); } }
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
    TableData { table_name: String::new(), columns: out_cols, rows: out_rows, sql_binding: None, sql_where: None, pending_op: None }
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


    fn translate_filter_to_sql(&self, func_idx: usize, cols: &[VMColumn], captures: &[Value]) -> Option<String> {
        let chunk = &self.ctx.functions[func_idx];
        if chunk.bytecode.len() > 40 { return None; } 
        
        // Track which register holds which value (column name or constant/captured value)
        let mut reg_values: HashMap<u8, (Option<String>, Option<Value>)> = HashMap::new();
        
        // The parameters: for where(), the first param is the row (R0).
        let row_reg = 0;

        // The captured values follow the parameters (R1, R2, ...).
        for (i, v) in captures.iter().enumerate() {
            reg_values.insert((i + 1) as u8, (None, Some(*v)));
        }

        let mut final_col = None;
        let mut final_op = None;
        let mut final_val = None;

        for instr in &*chunk.bytecode {
            match instr {
                OpCode::Move { dst, src } => {
                    if let Some(val) = reg_values.get(src).cloned() {
                        reg_values.insert(*dst, val);
                    }
                }
                OpCode::LoadConst { dst, idx } => {
                    let v = self.ctx.constants[*idx as usize];
                    reg_values.insert(*dst, (None, Some(v)));
                }
                OpCode::MethodCallCustom { dst, method_name_idx, base, arg_count, .. } if *arg_count == 0 => {
                    if *base == row_reg {
                        let name_val = self.ctx.constants[*method_name_idx as usize];
                        let name = String::from_utf8_lossy(&name_val.as_string()).into_owned();
                        if cols.iter().any(|c| c.name == name) {
                            reg_values.insert(*dst, (Some(name), None));
                        }
                    }
                }
                OpCode::Equal { src1, src2, .. } |
                OpCode::NotEqual { src1, src2, .. } |
                OpCode::Greater { src1, src2, .. } |
                OpCode::Less { src1, src2, .. } |
                OpCode::GreaterEqual { src1, src2, .. } |
                OpCode::LessEqual { src1, src2, .. } => {
                    let v1 = reg_values.get(src1);
                    let v2 = reg_values.get(src2);
                    
                    match (v1, v2) {
                        (Some((Some(col), None)), Some((None, Some(val)))) => {
                            final_col = Some(col.clone());
                            final_val = Some(*val);
                        }
                        (Some((None, Some(val))), Some((Some(col), None))) => {
                            final_col = Some(col.clone());
                            final_val = Some(*val);
                        }
                        _ => {}
                    }
                    
                    if final_col.is_some() {
                        final_op = match instr {
                            OpCode::Equal { .. } => Some("="),
                            OpCode::NotEqual { .. } => Some("<>"),
                            OpCode::Greater { .. } => Some(">"),
                            OpCode::Less { .. } => Some("<"),
                            OpCode::GreaterEqual { .. } => Some(">="),
                            OpCode::LessEqual { .. } => Some("<="),
                            _ => None,
                        };
                    }
                }
                _ => {}
            }
        }

        if let (Some(c), Some(o), Some(v)) = (final_col, final_op, final_val) {
            if v.is_int() || v.is_float() || v.is_bool() || v.is_string() {
                let v_str = if v.is_string() { format!("'{}'", v.to_string()) } else { v.to_string() };
                return Some(format!("[{}] {} {}", c, o, v_str));
            }
        }
        None
    }

    // ── method dispatch ───────────────────────────────────────────────────────
    #[inline(never)]
    fn handle_method_call<'a>(&mut self, dst: u8, receiver: Value, kind: MethodKind, args: &[Value], names: Option<&[String]>, ip: usize, locals: &mut [Value], vm_arc: &'a Arc<VM>, glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        if receiver.is_float() {
            match kind {
                MethodKind::ToStr => {
                    let res = Value::from_string(Arc::new(receiver.as_f64().to_string().into_bytes()));
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
                    let res = Value::from_string(Arc::new(receiver.as_i64().to_string().into_bytes()));
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
                    let res = Value::from_string(Arc::new(receiver.as_bool().to_string().into_bytes()));
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
                TAG_ARR  => self.handle_array_method(dst, receiver.as_array(), kind, args, ip, locals, vm_arc, glbs),
                TAG_SET  => self.handle_set_method(dst, receiver.as_set(), kind, args, ip, locals, vm_arc, glbs),
                TAG_MAP  => self.handle_map_method(dst, receiver.as_map(), kind, args, ip, locals, vm_arc, glbs),
                TAG_TBL  => self.handle_table_method(dst, receiver.as_table(), kind, args, names, ip, locals, vm_arc, glbs),
                TAG_ROW  => {
                    let rr = receiver.as_row();
                    self.handle_row_method(dst, rr, kind, ip, locals, vm_arc, glbs)
                }
                TAG_DATE => {
                    let d = chrono::DateTime::from_timestamp_millis(receiver.as_date()).unwrap().with_timezone(&chrono::Local).naive_local();
                    self.handle_date_method(dst, d, kind, args, ip, locals, vm_arc, glbs)
                }
                TAG_JSON => self.handle_json_method(dst, receiver.as_json(), kind, args, ip, locals, vm_arc, glbs),
                TAG_FIB  => self.handle_fiber_method(dst, receiver.as_fiber(), kind, args, ip, locals, vm_arc, glbs),
                TAG_STR  => self.handle_string_method(dst, &*receiver.as_string(), kind, args, ip, locals, vm_arc, glbs),
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



    fn handle_method_call_custom<'a>(&mut self, dst: u8, receiver: Value, method_name: &[u8], args: &[Value], ip: usize, locals: &mut [Value], vm_arc: &'a Arc<VM>, glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>, base: u8) -> OpResult {
        if receiver.is_ptr() {
            let tag = receiver.0 & 0x000F_0000_0000_0000;
            match tag {
                TAG_DB => {
                    self.handle_database_member_access(dst, receiver.as_db(), method_name, args, ip, locals, vm_arc, glbs)
                }
                TAG_JSON => self.handle_json_custom(dst, receiver.as_json(), method_name, args, ip, locals, vm_arc, glbs, base),
                TAG_ROW  => self.handle_row_custom(dst, receiver.as_row(), method_name, ip, locals, vm_arc, glbs),
                TAG_MAP  => {
                    let map_rc = receiver.as_map();
                    let map = map_rc.read();
                    let m_name_str = String::from_utf8_lossy(method_name);
                    
                    if m_name_str == "bind" && args.len() >= 1 {
                        let key_str = args[0].to_string();
                        let actual_key = if key_str.starts_with('/') { &key_str[1..] } else { &key_str };
                        
                        if let Some((_, v)) = map.iter().find(|(k, _)| k.to_string() == actual_key) {
                            let res = *v;
                            if args.len() >= 2 {
                                let target_reg = (base + 2) as usize; 
                                if target_reg < locals.len() {
                                    let old = locals[target_reg];
                                    if old.is_ptr() { unsafe { old.dec_ref(); } }
                                    if res.is_ptr() { unsafe { res.inc_ref(); } }
                                    locals[target_reg] = res;
                                }
                            }
                            unsafe { locals[dst as usize].dec_ref(); }
                            locals[dst as usize] = Value::from_bool(true);
                        } else {
                            unsafe { locals[dst as usize].dec_ref(); }
                            locals[dst as usize] = Value::from_bool(false);
                        }
                        OpResult::Continue
                    } else {
                        let res = if let Some((_, v)) = map.iter().find(|(k, _)| k.to_string() == m_name_str) {
                            unsafe { v.inc_ref(); }
                            *v
                        } else {
                            Value::from_bool(false)
                        };
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                        OpResult::Continue
                    }
                }
                _ => {
                    let m_name_str = String::from_utf8_lossy(method_name);
                    eprintln!("Method {} not found on pointer type {:x}{}", m_name_str, tag >> 48, self.current_span_info(ip));
                    OpResult::Halt
                }
            }
        } else {
            let m_name_str = String::from_utf8_lossy(method_name);
            eprintln!("Method {} not found on non-pointer type{}", m_name_str, self.current_span_info(ip));
            OpResult::Halt
        }
    }

    fn handle_database_member_access<'a>(&mut self, dst: u8, db_rc: Arc<DatabaseData>, member_name: &[u8], _args: &[Value], ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        let name_str = String::from_utf8_lossy(member_name);
        let tables = db_rc.tables.read();
        if let Some(table_val) = tables.get(name_str.as_ref()) {
            unsafe { table_val.inc_ref(); }
            unsafe { locals[dst as usize].dec_ref(); }
            locals[dst as usize] = *table_val;
            OpResult::Continue
        } else {
            eprintln!("Member {} not found on Database{}", name_str, self.current_span_info(ip));
            OpResult::Halt
        }
    }

    // ── fiber resume ──────────────────────────────────────────────────────────
    pub fn _resume_fiber<'a>(
        &mut self,
        fiber_rc: Arc<RwLock<FiberState>>,
        arg: Option<Value>,
        vm_arc: &'a Arc<VM>,
        glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>,
    ) -> Option<Value> {
        let mut current_arg = arg;
        loop {
            let ores = self.resume_fiber_with_guard(fiber_rc.clone(), current_arg, vm_arc, glbs);
            match ores {
                OpResult::Return(v) => return v,
                OpResult::Yield(v) => return v,
                OpResult::Call(target_f, call_arg, dst) => {
                    let sub_res = self._resume_fiber(target_f.clone(), call_arg, vm_arc, glbs);
                    
                    let _fiber_returned_value = target_f.read().is_done 
                        && sub_res.as_ref().map(|v| !v.is_bool_false()).unwrap_or(false);

                    if let Some(val) = sub_res {
                        let should_write = !target_f.read().is_done 
                            || val.is_ptr()  
                            || val.is_int()  
                            || (val.is_bool() && val.as_bool()); 
                        
                        if should_write {
                            let mut f = fiber_rc.write();
                            let d = dst as usize;
                            if d < f.locals.len() {
                                let old = f.locals[d];
                                if old.is_ptr() { unsafe { old.dec_ref(); } }
                                if val.is_ptr() { unsafe { val.inc_ref(); } }
                                f.locals[d] = val;
                            }
                        } else {
                            if val.is_ptr() { unsafe { val.dec_ref(); } }
                        }
                    }
                    current_arg = None;
                }
                _ => return None,
            }
        }
    }

    #[inline(never)]
    fn resume_fiber_with_guard<'a>(
        &mut self,
        fiber_rc: Arc<RwLock<FiberState>>,
        arg: Option<Value>,
        vm_arc: &'a Arc<VM>,
        glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>,
    ) -> OpResult {
        let (func_id, mut ip, mut locals) = {
            let mut f = fiber_rc.write();
            if f.is_done { 
                return OpResult::Return(None); 
            }
            if let Some(arg_val) = arg {
                if !f.locals.is_empty() {
                    let old = f.locals[0];
                    if old.is_ptr() { unsafe { old.dec_ref(); } }
                    if arg_val.is_ptr() { unsafe { arg_val.inc_ref(); } }
                    f.locals[0] = arg_val;
                }
            }
            (f.func_id, f.ip, std::mem::take(&mut f.locals))
        };
        let chunk = self.ctx.functions[func_id].clone();
        let old_spans = self.current_spans.replace(chunk.spans.clone());
        self.fiber_yielded = false;

        let ores = self.execute_bytecode_inner(
            &chunk.bytecode,
            &mut ip,
            &mut locals,
            vm_arc,
            glbs,
        );

        {
            let mut f = fiber_rc.write();
            f.ip = ip;
            f.locals = locals;
            if let OpResult::Return(_) = ores { f.is_done = true; }
        }
        self.current_spans = old_spans;
        ores
    }

    fn execute_trace(&self, trace: &Trace, ip: &mut usize, locals: &mut [Value], glbs: &mut RwLockWriteGuard<Vec<Value>>) -> Option<OpResult> {
        let native_ptr = trace.native_ptr.load(Ordering::Relaxed);
        
        if trace.min_locals > locals.len() {
            // In Fast-Path model, locals slice is already sized to chunk.max_locals.
            // We shouldn't need to resize here.
        }

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

                            let lv = if r == ir {
                                let r_val = locals.get_unchecked_mut(r);
                                if (r_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                    let v = (r_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                    r_val.0 = (r_val.0 & 0xFFFF_0000_0000_0000) | v;
                                    if v & 0x0000_8000_0000_0000 != 0 { (v | 0xFFFF_0000_0000_0000) as i64 } else { v as i64 }
                                } else {
                                    let v = r_val.as_i64().wrapping_add(1);
                                    *r_val = Value::from_i64(v);
                                    v
                                }
                            } else {
                                let ir_val = locals.get_unchecked_mut(ir);
                                if (ir_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                    let v = (ir_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                    ir_val.0 = (ir_val.0 & 0xFFFF_0000_0000_0000) | v;
                                } else {
                                    *ir_val = Value::from_i64(ir_val.as_i64().wrapping_add(1));
                                }
                                let r_val = locals.get_unchecked_mut(r);
                                if (r_val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                                    let v = (r_val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                                    r_val.0 = (r_val.0 & 0xFFFF_0000_0000_0000) | v;
                                    if v & 0x0000_8000_0000_0000 != 0 { (v | 0xFFFF_0000_0000_0000) as i64 } else { v as i64 }
                                } else {
                                    let v = r_val.as_i64().wrapping_add(1);
                                    *r_val = Value::from_i64(v);
                                    v
                                }
                            };

                            let limit_i64 = locals.get_unchecked(*limit_reg as usize).as_i64();
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
                        TraceOp::RandomInt { dst, min, max, step, has_step } => {
                            let mut rng = rand::rng();
                            let start = locals.get_unchecked(*min as usize).as_i64();
                            let end = locals.get_unchecked(*max as usize).as_i64();
                            let abs_diff = (end - start).abs();
                            let abs_step = if *has_step != 0 { locals.get_unchecked(*step as usize).as_i64().abs().max(1) } else { 1 };
                            let steps = abs_diff / abs_step;
                            let k = rng.random_range(0..=steps);
                            let sign = if end >= start { 1 } else { -1 };
                            *locals.get_unchecked_mut(*dst as usize) = Value::from_i64(start + k * sign * abs_step);
                        }
                        TraceOp::RandomFloat { dst, min, max, step, has_step, step_is_float } => {
                            let mut rng = rand::rng();
                            let start = locals.get_unchecked(*min as usize).as_f64();
                            let end = locals.get_unchecked(*max as usize).as_f64();
                            let diff = end - start;
                            let abs_diff = diff.abs();
                            let abs_step = if *has_step != 0 {
                                let s = locals.get_unchecked(*step as usize);
                                if *step_is_float { s.as_f64().abs() } else { s.as_i64().abs() as f64 }
                            } else { 0.5 };

                            if abs_step > 0.0 {
                                let steps = (abs_diff / abs_step).floor() as i64;
                                let k = rng.random_range(0..=steps);
                                let sign = if end >= start { 1.0 } else { -1.0 };
                                *locals.get_unchecked_mut(*dst as usize) = Value::from_f64(start + (k as f64) * sign * abs_step);
                            } else {
                                use rand::Rng;
                                let t: f64 = rng.random();
                                *locals.get_unchecked_mut(*dst as usize) = Value::from_f64(start + t * diff);
                            }
                        }
                        TraceOp::PowInt { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_i64();
                            let b = locals.get_unchecked(*src2 as usize).as_i64();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_i64(a.pow(b as u32));
                        }
                        TraceOp::PowFloat { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_f64();
                            let b = locals.get_unchecked(*src2 as usize).as_f64();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_f64(a.powf(b));
                        }
                        TraceOp::IntConcat { dst, src1, src2 } => {
                            let a = locals.get_unchecked(*src1 as usize).as_i64();
                            let b = locals.get_unchecked(*src2 as usize).as_i64();
                            let b_digits = if b == 0 { 1 } else { (b.abs() as f64).log10().floor() as u32 + 1 };
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_i64(a * 10i64.pow(b_digits) + b);
                        }
                        TraceOp::Has { dst, src1, src2 } => {
                            let container = *locals.get_unchecked(*src1 as usize);
                            let item = *locals.get_unchecked(*src2 as usize);
                            let res = if container.is_string() && item.is_string() {
                                let c_str = container.to_string();
                                let i_str = item.to_string();
                                Value::from_bool(c_str.contains(&i_str))
                            } else if container.is_array() {
                                let arc = container.as_array();
                                let ok = arc.read().iter().any(|v| v == &item);
                                Value::from_bool(ok)
                            } else if container.is_ptr() && (container.0 & 0x000F_0000_0000_0000) == TAG_SET {
                                let arc = container.as_set();
                                let ok = arc.read().elements.contains(&item);
                                Value::from_bool(ok)
                            } else if container.is_map() {
                                let arc = container.as_map();
                                let ok = arc.read().iter().any(|(k, _)| k == &item);
                                Value::from_bool(ok)
                            } else {
                                Value::from_bool(false)
                            };
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = res;
                        }
                        TraceOp::RandomChoice { dst, src } => {
                            let col = *locals.get_unchecked(*src as usize);
                            let res = if col.is_ptr() {
                                let mut rng = rand::rng();
                                match col.0 & 0x000F_0000_0000_0000 {
                                    TAG_ARR => {
                                        let arc = col.as_array();
                                        let arr = arc.read();
                                        if arr.is_empty() { Value::from_bool(false) }
                                        else { let v = arr[rng.random_range(0..arr.len())]; v.inc_ref(); v }
                                    }
                                    TAG_SET => {
                                        let arc = col.as_set();
                                        let mut s_write = arc.write();
                                        if s_write.cache.is_none() {
                                            s_write.cache = Some(s_write.elements.iter().cloned().collect());
                                        }
                                        let cache = s_write.cache.as_ref().unwrap();
                                        if cache.is_empty() { Value::from_bool(false) }
                                        else { let v = cache[rng.random_range(0..cache.len())]; v.inc_ref(); v }
                                    }
                                    _ => Value::from_bool(false),
                                }
                            } else { Value::from_bool(false) };
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = res;
                        }
                        TraceOp::ArraySize { dst, src } => {
                            let arr = *locals.get_unchecked(*src as usize);
                            let arc = arr.as_array();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_i64(arc.read().len() as i64);
                        }
                        TraceOp::ArrayGet { dst, arr_reg, idx_reg, fail_ip } => {
                            let arr = *locals.get_unchecked(*arr_reg as usize);
                            let idx = locals.get_unchecked(*idx_reg as usize).as_i64();
                            let arc = arr.as_array();
                            let arr_read = arc.read();
                            if idx < 0 || idx >= arr_read.len() as i64 {
                                *ip = *fail_ip;
                                return None;
                            }
                            let val = arr_read[idx as usize];
                            val.inc_ref();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = val;
                        }
                        TraceOp::ArrayPush { arr_reg, val_reg } => {
                            let arr = *locals.get_unchecked(*arr_reg as usize);
                            let val = *locals.get_unchecked(*val_reg as usize);
                            val.inc_ref();
                            let arc = arr.as_array();
                            arc.write().push(val);
                        }
                        TraceOp::SetSize { dst, src } => {
                            let set = *locals.get_unchecked(*src as usize);
                            let arc = set.as_set();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_i64(arc.read().elements.len() as i64);
                        }
                        TraceOp::SetContains { dst, set_reg, val_reg } => {
                            let set = *locals.get_unchecked(*set_reg as usize);
                            let val = *locals.get_unchecked(*val_reg as usize);
                            let arc = set.as_set();
                            let d = *dst as usize;
                            let old = locals.get_unchecked_mut(d);
                            if old.is_ptr() { old.dec_ref(); }
                            *old = Value::from_bool(arc.read().elements.contains(&val));
                        }
                        TraceOp::ArrayUpdate { arr_reg, idx_reg, val_reg, fail_ip } => {
                            let arr_val = *locals.get_unchecked(*arr_reg as usize);
                            let idx = locals.get_unchecked(*idx_reg as usize).as_i64();
                            let new_val = *locals.get_unchecked(*val_reg as usize);
                            let arc = arr_val.as_array();
                            let mut arr_write = arc.write();
                            if idx < 0 || idx >= arr_write.len() as i64 {
                                *ip = *fail_ip;
                                return None;
                            }
                            new_val.inc_ref();
                            let old = arr_write[idx as usize];
                            arr_write[idx as usize] = new_val;
                            old.dec_ref();
                        }
                    }
                }
                return None;
            }
        }
    }
    fn run_frame_owned(&mut self, chunk: FunctionChunk) -> Option<Value> {
        let vm_arc = self.vm.clone();
        let mut glbs = Some(vm_arc.globals.write());
        self.run_frame_owned_with_guard(chunk, &vm_arc, &mut glbs)
    }

    fn run_frame_owned_with_guard<'a>(&mut self, chunk: FunctionChunk, vm_arc: &'a Arc<VM>, glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> Option<Value> {
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
        let mut final_res = None;
        loop {
            let ores = self.execute_bytecode_inner(&chunk.bytecode, &mut ip, &mut locals, vm_arc, glbs);
            match ores {
                OpResult::Return(v) => { final_res = v; break; }
                OpResult::Call(target_f, arg, dst) => {
                    let sub_res = self._resume_fiber(target_f.clone(), arg, vm_arc, glbs);
                    let _fiber_returned_value = target_f.read().is_done 
                        && sub_res.as_ref().map(|v| !v.is_bool_false()).unwrap_or(false);

                    if let Some(val) = sub_res {
                        let should_write = !target_f.read().is_done 
                            || val.is_ptr()  
                            || val.is_int()  
                            || (val.is_bool() && val.as_bool()); 
                        
                        if should_write {
                            let d = dst as usize;
                            if d < locals.len() {
                                let old = locals[d];
                                if old.is_ptr() { unsafe { old.dec_ref(); } }
                                if val.is_ptr() { unsafe { val.inc_ref(); } }
                                locals[d] = val;
                            }
                        } else {
                            if val.is_ptr() { unsafe { val.dec_ref(); } }
                        }
                    }
                }
                OpResult::Continue | OpResult::Yield(_) | OpResult::Halt => break,
            }
        }
        self.hot_counts = old_hot;
        self.trace_cache = old_trace_cache;
        for v in locals { unsafe { v.dec_ref(); } }
        final_res
    }

    fn _run_frame(&mut self, func_id: usize, params: &[Value]) -> Option<Value> {
        let vm_arc = self.vm.clone();
        let mut glbs = Some(vm_arc.globals.write());
        let chunk = self.ctx.functions[func_id].clone();
        self.run_frame_with_guard(chunk, params, &vm_arc, &mut glbs, func_id)
    }

    fn run_frame_with_guard<'a>(&mut self, chunk: FunctionChunk, params: &[Value], vm_arc: &'a Arc<VM>, glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>, func_id: usize) -> Option<Value> {
        let old_spans = self.current_spans.replace(chunk.spans.clone());
        let mut ip = 0;
        
        let jit_ptr = chunk.jit_ptr.load(Ordering::Relaxed);
        if !jit_ptr.is_null() && !self.is_recording {
            let jit_fn: crate::backend::jit::MethodJitFunction = unsafe { std::mem::transmute(jit_ptr) };
            
            // Prepare locals
            let mut locals = self.locals_pool.pop().unwrap_or_else(|| Vec::with_capacity(chunk.max_locals.max(params.len())));
            locals.clear();
            for &v in params {
                unsafe { v.inc_ref(); }
                locals.push(v);
            }
            locals.resize(chunk.max_locals.max(params.len()), Value::from_bool(false));

            drop(glbs.take());
            let res_bits = unsafe {
                let mut glbs_lock = vm_arc.globals.write();
                let glbs_ptr = glbs_lock.as_mut_ptr();
                let consts = &self.ctx.constants;
                jit_fn(locals.as_mut_ptr(), glbs_ptr, consts.as_ptr(), Arc::as_ptr(vm_arc) as *mut VM, self as *mut Executor)
            };
            *glbs = Some(vm_arc.globals.write());
            
            self.locals_pool.push(locals);
            return Some(Value(res_bits));
        }

        // Trigger Method JIT
        if jit_ptr.is_null() && !chunk.has_loops && !chunk.has_terminal_ops && chunk.bytecode.len() < 500 {
            let count = chunk.call_count.fetch_add(1, Ordering::Relaxed);
            if count == 10 {
                let vm_copy = vm_arc.clone();
                let chunk_copy = chunk.clone();
                let func_id_copy = func_id;
                {
                    let mut jit = vm_copy.jit.lock();
                    match jit.compile_method(func_id_copy, &chunk_copy, &self.ctx.constants) {
                        Ok(ptr) => {
                            chunk_copy.jit_ptr.store(ptr as *mut u8, Ordering::Release);
                        }
                        Err(_) => {}
                    }
                }
            }
        }

        // [XCX 3.0 Fast-Path Pooling] Retrieve or create vectors for this frame
        let mut locals = self.locals_pool.pop().unwrap_or_else(|| Vec::with_capacity(chunk.max_locals.max(params.len())));
        locals.clear();
        for &v in params {
            unsafe { v.inc_ref(); }
            locals.push(v);
        }
        locals.resize(chunk.max_locals.max(params.len()), Value::from_bool(false));

        let mut hot_counts = if chunk.has_loops {
            self.hot_counts_pool.pop().unwrap_or_else(|| vec![0; chunk.bytecode.len()])
        } else {
            Vec::new()
        };
        if !hot_counts.is_empty() {
            if hot_counts.len() != chunk.bytecode.len() {
                hot_counts.resize(chunk.bytecode.len(), 0);
            }
            hot_counts.fill(0);
        }

        let mut trace_cache = if chunk.has_loops {
            self.trace_cache_pool.pop().unwrap_or_else(|| vec![None; chunk.bytecode.len()])
        } else {
            Vec::new()
        };

        if !trace_cache.is_empty() {
            if trace_cache.len() != chunk.bytecode.len() {
                trace_cache.resize(chunk.bytecode.len(), None);
                trace_cache.fill(None);
            } else {
                let current_rev = self.vm.jit_revision.load(Ordering::Relaxed);
                if self.trace_revision < current_rev {
                    let traces = self.vm.traces.read();
                    for (tip, trace) in traces.iter() {
                         if *tip < trace_cache.len() {
                             trace_cache[*tip] = Some(trace.clone());
                         }
                    }
                    self.trace_revision = current_rev;
                }
            }
        }

        let old_hot = std::mem::replace(&mut self.hot_counts, hot_counts);
        let old_trace_cache = std::mem::replace(&mut self.trace_cache, trace_cache);

        self.recording_trace = None;
        self.is_recording = false;
        let mut final_res = None;

        loop {
            let ores = self.execute_bytecode_inner(&chunk.bytecode, &mut ip, &mut locals, vm_arc, glbs);
            match ores {
                OpResult::Return(v) => { final_res = v; break; }
                OpResult::Call(target_f, arg, dst) => {
                    let sub_res = self._resume_fiber(target_f.clone(), arg, vm_arc, glbs);
                    if let Some(val) = sub_res {
                        let should_write = !target_f.read().is_done 
                            || val.is_ptr()  
                            || val.is_int()  
                            || (val.is_bool() && val.as_bool()); 
                        
                        if should_write {
                            let d = dst as usize;
                            if d < locals.len() {
                                let old = locals[d];
                                if old.is_ptr() { unsafe { old.dec_ref(); } }
                                if val.is_ptr() { unsafe { val.inc_ref(); } }
                                locals[d] = val;
                            }
                        } else {
                            if val.is_ptr() { unsafe { val.dec_ref(); } }
                        }
                    }
                }
                OpResult::Yield(_) | OpResult::Halt | OpResult::Continue => { break; }
            }
        }

        // [XCX 3.0 Fast-Path Pooling] Restore metadata and return vectors to pool
        let hot_counts = std::mem::replace(&mut self.hot_counts, old_hot);
        let trace_cache = std::mem::replace(&mut self.trace_cache, old_trace_cache);
        
        self.hot_counts_pool.push(hot_counts);
        self.trace_cache_pool.push(trace_cache);

        for v in locals.iter() {
             unsafe { v.dec_ref(); }
        }
        self.locals_pool.push(locals);

        self.current_spans = old_spans;
        final_res
    }

    #[inline(never)]
    fn execute_bytecode_extended<'a>(&mut self, op: OpCode, ip: &mut usize, locals: &mut [Value], vm_arc: &'a Arc<VM>, glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        match op {
            OpCode::Input { dst, ty } => {
                use std::io::BufRead;
                let _ = std::io::stdout().flush();
                // Release globals lock before blocking on stdin
                drop(glbs.take());
                let mut line = String::new();
                let stdin = std::io::stdin();
                let _ = stdin.lock().read_line(&mut line);
                *glbs = Some(vm_arc.globals.write());
                let trimmed = line.trim_end_matches(['\n', '\r']);
                
                let val = match ty {
                    TypeTag::Int => {
                        if trimmed.contains('.') {
                            eprintln!("R103: Error: Type mismatch - expected integer, got float at input{}", self.current_span_info(*ip));
                            return OpResult::Halt;
                        }
                        if let Ok(n) = trimmed.parse::<i64>() {
                            Value::from_i64(n)
                        } else {
                            eprintln!("R103: Error: Type mismatch - expected integer, got '{}' at input{}", trimmed, self.current_span_info(*ip));
                            return OpResult::Halt;
                        }
                    }
                    TypeTag::Float => {
                        if let Ok(f) = trimmed.parse::<f64>() {
                            Value::from_f64(f)
                        } else {
                            eprintln!("R103: Error: Type mismatch - expected float, got '{}' at input{}", trimmed, self.current_span_info(*ip));
                            return OpResult::Halt;
                        }
                    }
                    TypeTag::Bool => {
                        if trimmed == "true" {
                            Value::from_bool(true)
                        } else if trimmed == "false" {
                            Value::from_bool(false)
                        } else {
                            eprintln!("R103: Error: Type mismatch - expected boolean, got '{}' at input{}", trimmed, self.current_span_info(*ip));
                            return OpResult::Halt;
                        }
                    }
                    TypeTag::String => {
                        Value::from_string(std::sync::Arc::new(trimmed.to_string().into_bytes()))
                    }
                    TypeTag::Unknown => {
                        // Maintain legacy fallback for unknown types
                        if let Ok(n) = trimmed.parse::<i64>() {
                            Value::from_i64(n)
                        } else if let Ok(f) = trimmed.parse::<f64>() {
                            Value::from_f64(f)
                        } else if trimmed == "true" {
                            Value::from_bool(true)
                        } else if trimmed == "false" {
                            Value::from_bool(false)
                        } else {
                            Value::from_string(std::sync::Arc::new(trimmed.to_string().into_bytes()))
                        }
                    }
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = val;
                OpResult::Continue
            }
            OpCode::TerminalRun { dst, cmd_src } => {
                let cmd = locals[cmd_src as usize].to_string();
                drop(glbs.take());

                // [XCX 3.0] Refactored to be Cargo-independent.
                // We use the current executable to run the command, which allows running
                // scripts in any directory without requiring a Cargo.toml.
                let status = if let Ok(exe) = std::env::current_exe() {
                    std::process::Command::new(exe).arg(&cmd).status()
                } else {
                    std::process::Command::new("xcx").arg(&cmd).status()
                };

                *glbs = Some(vm_arc.globals.write());
                let success = status.map(|s| s.success()).unwrap_or(false);
                let res = Value::from_bool(success);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::TerminalClear => {
                let mut out = std::io::stdout();
                let _ = execute!(out, Clear(ClearType::All), MoveTo(0, 0));
                let _ = out.flush();
                OpResult::Continue
            }
            OpCode::TerminalRaw => {
                if let Err(_) = enable_raw_mode() {
                    eprintln!("R440: Error: Failed to set terminal mode{}", self.current_span_info(*ip));
                    return OpResult::Halt;
                }
                self.terminal_raw_enabled = true;
                OpResult::Continue
            }
            OpCode::TerminalNormal => {
                let _ = disable_raw_mode();
                self.terminal_raw_enabled = false;
                OpResult::Continue
            }
            OpCode::TerminalCursor { on } => {
                let mut out = std::io::stdout();
                if on { let _ = execute!(out, Show); }
                else { let _ = execute!(out, Hide); }
                let _ = out.flush();
                OpResult::Continue
            }
            OpCode::TerminalMove { x_src, y_src } => {
                let x = locals[x_src as usize].as_i64();
                let y = locals[y_src as usize].as_i64();
                
                if x < 0 || y < 0 || x > 32767 || y > 32767 {
                     eprintln!("R441: Error: Cursor position out of bounds (x:{}, y:{}){}", x, y, self.current_span_info(*ip));
                     return OpResult::Halt;
                }

                let mut out = std::io::stdout();
                if let Err(_) = execute!(out, MoveTo(x as u16, y as u16)) {
                     eprintln!("R441: Error: Cursor position out of bounds{}", self.current_span_info(*ip));
                     return OpResult::Halt;
                }
                let _ = out.flush();
                OpResult::Continue
            }
            OpCode::TerminalWrite { src } => {
                print!("{}", locals[src as usize].to_string());
                let _ = std::io::stdout().flush();
                OpResult::Continue
            }
            OpCode::TerminalExit => {
                let _ = disable_raw_mode();
                std::process::exit(0);
            }
            OpCode::InputKey { dst } => {
                if !self.terminal_raw_enabled {
                    eprintln!("R442: Alert: input.key() called outside !raw mode{}", self.current_span_info(*ip));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_string(Arc::new(vec![]));
                    return OpResult::Continue;
                }

                let res = match event::poll(std::time::Duration::from_millis(0)) {
                    Ok(true) => {
                        match event::read() {
                            Ok(Event::Key(KeyEvent { code, kind, .. })) => {
                                if kind == KeyEventKind::Press {
                                    map_key_code_to_value(code)
                                } else {
                                    Value::from_bool(false)
                                }
                            }
                            Ok(_) => Value::from_bool(false),
                            Err(_) => {
                                eprintln!("R443: Error: Failed to read input{}", self.current_span_info(*ip));
                                return OpResult::Halt;
                            }
                        }
                    }
                    Ok(false) => Value::from_bool(false),
                    Err(_) => {
                        eprintln!("R443: Error: Failed to read input{}", self.current_span_info(*ip));
                        return OpResult::Halt;
                    }
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::InputKeyWait { dst } => {
                if !self.terminal_raw_enabled {
                    eprintln!("R442: Alert: input.key() called outside !raw mode{}", self.current_span_info(*ip));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_string(Arc::new(vec![]));
                    return OpResult::Continue;
                }

                drop(glbs.take());
                let res = loop {
                    match event::read() {
                        Ok(Event::Key(KeyEvent { code, kind, .. })) => {
                            if kind == KeyEventKind::Press {
                                break map_key_code_to_value(code);
                            }
                            continue;
                        }
                        Ok(_) => continue,
                        Err(_) => {
                            eprintln!("R443: Error: Failed to read input{}", self.current_span_info(*ip));
                            return OpResult::Halt;
                        }
                    }
                };
                *glbs = Some(vm_arc.globals.write());
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::InputReady { dst } => {
                let ready = event::poll(std::time::Duration::from_millis(0)).unwrap_or(false);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = Value::from_bool(ready);
                OpResult::Continue
            }
            OpCode::DateNow { dst } => {
                let now = chrono::Local::now().timestamp_millis();
                let res = Value::from_date(now);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::JsonParse { dst, src } => {
                let s = locals[src as usize].to_string();
                let res = if s.is_empty() {
                    Value::from_bool(false)
                } else {
                    match serde_json::from_str::<serde_json::Value>(&s) {
                        Ok(j) => json_serde_to_value(&j),
                        Err(e) => {
                            let preview = if s.len() > 50 { format!("{}...", &s[..50]) } else { s.clone() };
                            eprintln!("R305: Error: Invalid JSON - {}. Input: {:?}{}", e, preview, self.current_span_info(*ip));
                            return OpResult::Halt;
                        }
                    }
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::JsonBindLocal { dst, json_src, path_src } => {
                let json_val = locals[json_src as usize];
                let path_val = locals[path_src as usize];
                let path = path_val.to_string();
                let res = get_path_value_xcx(json_val, &path);
                
                if res.is_ptr() { unsafe { res.inc_ref(); } }
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::JsonBind { idx, json_src, path_src } => {
                let json_val = locals[json_src as usize];
                let path_val = locals[path_src as usize];
                let path = path_val.to_string();
                let val = get_path_value_xcx(json_val, &path);
                
                let idx = idx as usize;
                // glbs is already locked by the caller (execute_bytecode_inner)
                let g_mut = glbs.as_mut().expect("Globals lock lost");
                if idx >= g_mut.len() { g_mut.resize(idx + 1, Value::from_bool(false)); }
                
                if val.is_ptr() { unsafe { val.inc_ref(); } }
                unsafe { g_mut[idx].dec_ref(); }
                g_mut[idx] = val;
                OpResult::Continue
            }
            OpCode::JsonInjectLocal { table_reg, json_src, mapping_src } => {
                let table = locals[table_reg as usize];
                let json = locals[json_src as usize];
                let mapping = locals[mapping_src as usize];
                if json.is_map() || json.is_array() {
                    self.native_inject_table(&table, json, &mapping);
                } else {
                    self.json_inject_table(&table, &json, &mapping);
                }
                OpResult::Continue
            }
            OpCode::JsonInject { table_idx, json_src, mapping_src } => {
                let json = locals[json_src as usize];
                let mapping = locals[mapping_src as usize];
                let table = self.vm.globals.read()[table_idx as usize];
                if json.is_map() || json.is_array() {
                    self.native_inject_table(&table, json, &mapping);
                } else {
                    self.json_inject_table(&table, &json, &mapping);
                }
                OpResult::Continue
            }
            OpCode::CryptoHash { dst, pass_src, alg_src } => {
                let pass_val = locals[pass_src as usize];
                let algo = locals[alg_src as usize].to_string();
                
                let raw_bytes: Vec<u8> = if pass_val.is_string() {
                    (*pass_val.as_string()).clone()
                } else {
                    pass_val.to_string().into_bytes()
                };
                
                let bytes = &raw_bytes;
                let res = match algo.as_str() {
                    "bcrypt" => {
                        let password = String::from_utf8_lossy(bytes);
                        bcrypt::hash(&*password, bcrypt::DEFAULT_COST).map(|h| Value::from_string(Arc::new(h.into_bytes()))).unwrap_or(Value::from_bool(false))
                    }
                    "argon2" => {
                        use argon2::PasswordHasher;
                        let mut salt_bytes = [0u8; 16];
                        rand::fill(&mut salt_bytes);
                        let salt = argon2::password_hash::SaltString::encode_b64(&salt_bytes).unwrap();
                        let argon2 = argon2::Argon2::default();
                        argon2.hash_password(bytes, &salt)
                            .map(|h| Value::from_string(Arc::new(h.to_string().into_bytes())))
                            .unwrap_or(Value::from_bool(false))
                    }
                    "base64_encode" => {
                        use base64::Engine;
                        let encoded = base64::engine::general_purpose::STANDARD.encode(&*bytes);
                        Value::from_string(Arc::new(encoded.into_bytes()))
                    }
                    "base64_decode" => {
                        use base64::Engine;
                        let s = String::from_utf8_lossy(&bytes);
                        match base64::engine::general_purpose::STANDARD.decode(s.as_ref()) {
                            Ok(decoded) => Value::from_string(Arc::new(decoded)),
                            Err(_) => Value::from_bool(false),
                        }
                    }
                    _ => Value::from_bool(false),
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::CryptoVerify { dst, pass_src, hash_src, alg_src } => {
                let password = locals[pass_src as usize].to_string();
                let hashed = locals[hash_src as usize].to_string();
                let algo = locals[alg_src as usize].to_string();
                
                let ok = match algo.as_str() {
                    "bcrypt" => bcrypt::verify(&*password, &*hashed).unwrap_or(false),
                    "argon2" => {
                        if let Ok(parsed_hash) = PasswordHash::new(&hashed) {
                            let verify_res = argon2::Argon2::default().verify_password(password.as_bytes(), &parsed_hash);
                            verify_res.is_ok()
                        } else { 
                            false 
                        }
                    }
                    _ => false,
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = Value::from_bool(ok);
                OpResult::Continue
            }
            OpCode::CryptoToken { dst, len_src } => {
                let len = locals[len_src as usize].as_i64() as usize;
                let token: String = (0..len).map(|_| {
                    const CHARSET: &[u8] = b"0123456789abcdef";
                    let idx = rand::rng().random_range(0..CHARSET.len());
                    CHARSET[idx] as char
                }).collect();
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = Value::from_string(Arc::new(token.into_bytes()));
                OpResult::Continue
            }
            OpCode::HttpCall { dst, method_idx, url_src, body_src } => {
                let url = locals[url_src as usize].to_string();
                let body_val = locals[body_src as usize];
                let method = self.ctx.constants[method_idx as usize].to_string();
                
                if url.contains("169.254.") || url.contains("instance-data") {
                    let mut map = serde_json::Map::new();
                    map.insert("ok".to_string(), serde_json::Value::Bool(false));
                    map.insert("error".to_string(), serde_json::Value::String("SSRF attempt blocked".to_string()));
                    let res = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(map))));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                    return OpResult::Continue;
                }
                
                let res = match method.to_uppercase().as_str() {
                    "GET" => ureq::get(&url).call(),
                    "POST" => {
                        if body_val.is_string() {
                            ureq::post(&url).send_bytes(&*body_val.as_string())
                        } else {
                            ureq::post(&url).send_string(&body_val.to_string())
                        }
                    }
                    _ => ureq::get(&url).call(),
                };
                let val = Value::from_json(Arc::new(RwLock::new(build_response_json(res))));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = val;
                OpResult::Continue
            }
            OpCode::HttpRequest { dst, arg_src } => {
                let arg_val = locals[arg_src as usize];
                if arg_val.is_map() {
                    let map_rc = arg_val.as_map();
                    let map = map_rc.read();
                    
                    let method = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == b"method").map(|(_, v)| v.to_string()).unwrap_or_else(|| "GET".to_string());
                    let url = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == b"url").map(|(_, v)| v.to_string()).unwrap_or_default();
                    
                    if let Err(e) = is_safe_url(&url) {
                        let mut res_map = serde_json::Map::new();
                        res_map.insert("ok".to_string(), serde_json::Value::Bool(false));
                        res_map.insert("error".to_string(), serde_json::Value::String(e));
                        let val = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(res_map))));
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = val;
                    } else {
                        drop(map);
                        
                        let mut request = match method.to_uppercase().as_str() {
                            "POST" => ureq::post(&url),
                            "PUT" => ureq::put(&url),
                            "DELETE" => ureq::delete(&url),
                            "PATCH" => ureq::patch(&url),
                            "HEAD" => ureq::head(&url),
                            _ => ureq::get(&url),
                        };
                        
                        let map = map_rc.read();
                        if let Some((_, h_val)) = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == b"headers") {
                            if h_val.is_map() {
                                let h_map_rc = h_val.as_map();
                                let h_map = h_map_rc.read();
                                for (k, v) in h_map.iter() {
                                     let k_str = k.to_string();
                                     let v_str = v.to_string();
                                     request = request.set(&k_str, &v_str);
                                }
                            } else if h_val.is_array() {
                                let h_arr_rc = h_val.as_array();
                                let h_arr = h_arr_rc.read();
                                for pair in h_arr.iter() {
                                    if pair.is_map() {
                                        let p_map_rc = pair.as_map();
                                        let p_map = p_map_rc.read();
                                        for (pk, pv) in p_map.iter() {
                                            let pk_str = pk.to_string();
                                            let pv_str = pv.to_string();
                                            request = request.set(&pk_str, &pv_str);
                                        }
                                    } else {
                                        let pair_str = pair.to_string();
                                        if let Some(idx) = pair_str.find(" :: ") {
                                            let k = pair_str[..idx].trim();
                                            let v = pair_str[idx+4..].trim();
                                            request = request.set(k, v);
                                        }
                                    }
                                }
                            }
                        }
                        
                        if let Some((_, t_val)) = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == b"timeout") {
                            if t_val.is_int() {
                                request = request.timeout(std::time::Duration::from_millis(t_val.as_i64() as u64));
                            }
                        }
                        
                        let body_val = map.iter().find(|(k, _)| k.is_string() && *k.as_string() == b"body").map(|(_, v)| *v);
                        let response = if let Some(b) = body_val {
                            if b.is_string() {
                                request.send_bytes(&*b.as_string())
                            } else {
                                request.send_string(&b.to_string())
                            }
                        } else {
                            request.call()
                        };
                        
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
                OpResult::Continue
            }
            OpCode::StoreAppend { base } => {
                let path_str = locals[base as usize].to_string();
                let path = std::path::Path::new(&path_str);
                let content_val = locals[(base + 1) as usize];
                let content = content_val.as_string();
                use std::io::Write as _;
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(path) {
                    let _ = f.write_all(&content);
                }
                OpResult::Continue
            }
            OpCode::StoreExists { dst, base } => {
                let path_str = locals[base as usize].to_string();
                let path = std::path::Path::new(&path_str);
                let exists = path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false);
                let res = Value::from_bool(exists);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::StoreDelete { base } => {
                let path = locals[base as usize].to_string();
                let _ = std::fs::remove_file(path);
                OpResult::Continue
            }
            OpCode::StoreList { dst, base } => {
                let path_str = locals[base as usize].to_string();
                validate_path_safety(&path_str);
                let mut files = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&path_str) {
                    for entry in entries.flatten() {
                        if let Ok(name) = entry.file_name().into_string() {
                            files.push(Value::from_string(Arc::new(name.into_bytes())));
                        }
                    }
                }
                let res = Value::from_array(Arc::new(RwLock::new(files)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::StoreIsDir { dst, base } => {
                let path_str = locals[base as usize].to_string();
                validate_path_safety(&path_str);
                let res = Value::from_bool(std::path::Path::new(&path_str).is_dir());
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::StoreSize { dst, base } => {
                let path_str = locals[base as usize].to_string();
                validate_path_safety(&path_str);
                let size = std::fs::metadata(&path_str).map(|m| m.len()).unwrap_or(0);
                let res = Value::from_i64(size as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::StoreMkdir { base } => {
                let path_str = locals[base as usize].to_string();
                validate_path_safety(&path_str);
                let _ = std::fs::create_dir_all(path_str);
                OpResult::Continue
            }
            OpCode::StoreGlob { dst, base } => {
                let pattern = locals[base as usize].to_string();
                validate_path_safety(&pattern);
                let mut results = Vec::new();
                if let Ok(paths) = glob::glob(&pattern) {
                    for entry in paths.flatten() {
                        if let Some(s) = entry.to_str() {
                            results.push(Value::from_string(Arc::new(s.to_string().into_bytes())));
                        }
                    }
                }
                let res = Value::from_array(Arc::new(RwLock::new(results)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::StoreZip { dst, base } => {
                let source = locals[base as usize].to_string();
                let target = locals[(base + 1) as usize].to_string();
                validate_path_safety(&source);
                validate_path_safety(&target);
                let ok = zip_folder(&source, &target).is_ok();
                let res = Value::from_bool(ok);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::StoreUnzip { dst, base } => {
                let zip_file = locals[base as usize].to_string();
                let dest_dir = locals[(base + 1) as usize].to_string();
                validate_path_safety(&zip_file);
                validate_path_safety(&dest_dir);
                let ok = unzip_archive(&zip_file, &dest_dir).is_ok();
                let res = Value::from_bool(ok);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::DatabaseInit { dst, engine_src, path_src, tables_base_reg, table_count } => {
                let engine = locals[engine_src as usize].to_string();
                let path = locals[path_src as usize].to_string();
                if engine != "sqlite" {
                    eprintln!("R300: Unsupported database engine: {}{}", engine, self.current_span_info(*ip));
                    return OpResult::Halt;
                }
                let conn = match rusqlite::Connection::open(&path) {
                    Ok(c) => Arc::new(Mutex::new(c)),
                    Err(e) => {
                        eprintln!("R300: Failed to open database {}: {}{}", path, e, self.current_span_info(*ip));
                        return OpResult::Halt;
                    }
                };

                let table_map = Arc::new(RwLock::new(HashMap::new()));
                let tables_start = tables_base_reg as usize;
                for i in 0..table_count {
                    let name_val = locals[tables_start + (i as usize * 2)];
                    let table_val = locals[tables_start + (i as usize * 2) + 1];
                    let name = name_val.to_string();

                    if table_val.is_ptr() && (table_val.0 & 0x000F_0000_0000_0000) == TAG_TBL {
                        unsafe { table_val.inc_ref(); }
                        table_map.write().insert(name.clone(), table_val);
                        
                        let t_rc = table_val.as_table();
                        let mut t = t_rc.write();
                        t.sql_binding = Some(crate::backend::vm::SqlBinding {
                            db_conn: conn.clone(),
                            table_name: name.clone(),
                        });
                        
                        // Sync schema: CREATE TABLE IF NOT EXISTS
                        let mut sql = format!("CREATE TABLE IF NOT EXISTS [{}] (", name);
                        let mut first = true;
                        for col in &t.columns {
                            if !first { sql.push_str(", "); }
                            first = false;
                            sql.push_str(&format!("[{}]", col.name));
                            
                            // Map XCX types to SQLite types
                            match col.ty {
                                crate::parser::ast::Type::Int => sql.push_str(" INTEGER"),
                                crate::parser::ast::Type::Float => sql.push_str(" REAL"),
                                crate::parser::ast::Type::Bool => sql.push_str(" INTEGER"),
                                _ => sql.push_str(" TEXT"),
                            }
                            
                            if col.is_auto { sql.push_str(" PRIMARY KEY AUTOINCREMENT"); }
                        }
                        sql.push(')');
                        
                        let c = conn.lock();
                        if let Err(e) = c.execute(&sql, []) {
                            eprintln!("R300: Failed to sync schema for table {}: {}{}", name, e, self.current_span_info(*ip));
                        }
                    }
                }

                let db_data = DatabaseData {
                    conn,
                    engine,
                    path,
                    tables: table_map,
                };
                let res = Value::from_db(Arc::new(db_data));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
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
                                let local_idx = base as usize + (r as usize * non_auto_count) + data_idx;
                                if local_idx >= locals.len() {
                                    return OpResult::Halt;
                                }
                                let v = locals[local_idx];
                                unsafe { v.inc_ref(); }
                                row_vals.push(v);
                                data_idx += 1;
                            }
                        }
                        rows.push(row_vals);
                    }
                    let res = Value::from_table(Arc::new(RwLock::new(TableData { table_name: String::new(), columns: col_def, rows, sql_binding: None, sql_where: None, pending_op: None })));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpResult::Continue
            }
            OpCode::StoreRead { dst, base } => {
                let path = locals[base as usize].to_string();
                let res = std::fs::read(path).map(|b| Value::from_string(Arc::new(b))).unwrap_or(Value::from_bool(false));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::StoreWrite { base } => {
                let path_str = locals[base as usize].to_string();
                let path = std::path::Path::new(&path_str);
                let content_val = locals[(base + 1) as usize];
                let content = content_val.as_string();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(path, &*content);
                OpResult::Continue
            }
            OpCode::HaltAlert { src } => {
                println!("ALERT: {}", locals[src as usize].to_string());
                OpResult::Continue
            }
            OpCode::HaltError { src } => {
                eprintln!("ERROR: {}{}", locals[src as usize].to_string(), self.current_span_info(*ip));
                self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                OpResult::Halt
            }
            OpCode::HaltFatal { src } => {
                eprintln!("FATAL: {}{}", locals[src as usize].to_string(), self.current_span_info(*ip));
                self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                OpResult::Halt
            }
            OpCode::MethodCallNamed { dst, kind, base, arg_count, names_idx } => {
                let receiver = locals[base as usize];
                let args: Vec<Value> = locals[(base as usize + 1)..(base as usize + 1 + arg_count as usize)].to_vec();
                let names_val = self.ctx.constants[names_idx as usize];
                let names_arr = names_val.as_array();
                let names: Vec<String> = names_arr.read().iter().map(|v| String::from_utf8_lossy(&v.as_string()).into_owned()).collect();

                let ores = if receiver.is_db() {
                    self.handle_database_method(dst, receiver.as_db(), kind, &args, Some(&names), *ip, locals, vm_arc, glbs)
                } else {
                    self.handle_method_call(dst, receiver, kind, &args, Some(&names), *ip, locals, vm_arc, glbs)
                };
                ores
            }
            OpCode::MethodCallCustom { dst, method_name_idx, base, arg_count } => {
                let receiver = locals[base as usize];
                let method_name = self.ctx.constants[method_name_idx as usize].as_string();
                let args: Vec<Value> = locals[(base as usize + 1)..(base as usize + 1 + arg_count as usize)].to_vec();
                let ores = self.handle_method_call_custom(dst, receiver, &*method_name, &args, *ip, locals, vm_arc, glbs, base);
                ores
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
                    trace_revision: 0,
                };
                for v in &f.locals { unsafe { v.inc_ref(); } }
                let res = Value::from_fiber(Arc::new(RwLock::new(f)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }

            OpCode::HttpRespond { status_src, body_src, headers_src } => {
                let status = locals[status_src as usize].as_i64() as u32;
                let body_val = locals[body_src as usize];
                let (body_bytes, _is_binary) = if body_val.is_ptr() && (body_val.0 & 0x000F_0000_0000_0000) == TAG_STR {
                    (body_val.as_string(), true)
                } else if body_val.is_ptr() && (body_val.0 & 0x000F_0000_0000_0000) == TAG_JSON {
                    (Arc::new(body_val.as_json().read().to_string().into_bytes()), false)
                } else if body_val.is_ptr() && ((body_val.0 & 0x000F_0000_0000_0000) == TAG_ARR || 
                                             (body_val.0 & 0x000F_0000_0000_0000) == TAG_MAP ||
                                             (body_val.0 & 0x000F_0000_0000_0000) == TAG_TBL) {
                    (Arc::new(value_to_json(&body_val).to_string().into_bytes()), false)
                } else {
                    (Arc::new(body_val.to_string().into_bytes()), false)
                };
                let headers = locals[headers_src as usize];
                
                if let Some(req_mutex_arc) = self.http_req.clone() {
                    let mut req_opt = req_mutex_arc.lock();
                    if let Some(request) = req_opt.take() {
                        let mut response = tiny_http::Response::from_data(&body_bytes[..])
                            .with_status_code(status);
                        
                        let mut ct_set = false;
                        let mut origin_set = false;
                        let mut methods_set = false;
                        let mut headers_set = false;
                        
                        if headers.is_array() {
                            let arr_rc = headers.as_array();
                            let arr = arr_rc.read();
                            for item in arr.iter() {
                                if item.is_map() {
                                    let map_rc = item.as_map();
                                    let map = map_rc.read();
                                    for (k, v) in map.iter() {
                                        let ks = k.to_string();
                                        let ks_low = ks.to_lowercase();
                                        let vs = v.to_string();
                                        if ks_low == "content-type" { ct_set = true; }
                                        if ks_low == "access-control-allow-origin" { origin_set = true; }
                                        if ks_low == "access-control-allow-methods" { methods_set = true; }
                                        if ks_low == "access-control-allow-headers" { headers_set = true; }
                                        
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
                        
                        if !origin_set {
                            response = response.with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                        }
                        if !methods_set {
                            response = response.with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS, DELETE, PATCH"[..]).unwrap());
                        }
                        if !headers_set {
                            response = response.with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type, Authorization, X-CSRF-TOKEN"[..]).unwrap());
                        }
                        
                        let _ = request.respond(response);
                    }
                }
                OpResult::Yield(None)
            }
            OpCode::HttpServe { func_idx: _, port_src, host_src, workers_src, routes_src } => {
                let port = locals[port_src as usize].as_i64() as u16;
                let host_raw = locals[host_src as usize].to_string();
                let host = if host_raw == "false" || host_raw.is_empty() { "127.0.0.1".to_string() } else { host_raw };
                let workers_val = locals[workers_src as usize].as_i64();
                let workers = if workers_val <= 0 { 4 } else { workers_val as usize };
                let routes_val = locals[routes_src as usize];
                
                let addr = format!("{}:{}", host, port);
                let server = Arc::new(tiny_http::Server::http(&addr).expect("Failed to start server"));
                let mut routes = Vec::new();
                if routes_val.is_array() {
                    let arr_rc = routes_val.as_array();
                    let arr = arr_rc.read();
                    for (idx, item) in arr.iter().enumerate() {
                        if item.is_map() {
                            let map_rc = item.as_map();
                            let map = map_rc.read();
                            for (k, v) in map.iter() {
                                 if v.is_func() {
                                     let fid = v.as_function() as usize;
                                     routes.push((k.to_string(), fid));
                                 } else if v.is_fiber() {
                                     let fid = v.as_fiber().read().func_id;
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

                
                drop(glbs.take());
                let routes = Arc::new(routes);
                for i in 0..workers {
                    let server = server.clone();
                    let routes = routes.clone();
                    let vm = vm_arc.clone();
                    let ctx = self.ctx.clone();
                    let terminal_raw = self.terminal_raw_enabled;
                    let _ = std::thread::Builder::new()
                        .name(format!("xcx-worker-{}", i))
                        .stack_size(32 * 1024 * 1024)
                        .spawn(move || {
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
                                
                                let rt_key_norm = route_key.split_whitespace().collect::<Vec<_>>().join(" ");
                                let handler_idx = routes.iter()
                                    .find(|(r, _)| {
                                        let r_norm = r.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
                                        r_norm == rt_key_norm || *r == "*" 
                                    })
                                    .map(|(_, idx)| *idx);
                                
                                if let Some(fid) = handler_idx {
                                    let mut body_bytes = Vec::new();
                                    let _ = request.as_reader().read_to_end(&mut body_bytes);
                                    
                                    let mut req_map = serde_json::Map::new();
                                    req_map.insert("method".into(), method.into());
                                    req_map.insert("url".into(), url.into());
                                    let body_str = String::from_utf8_lossy(&body_bytes);
                                    let body_json: serde_json::Value = serde_json::from_str(&body_str).unwrap_or_else(|_| {
                                        if body_bytes.is_empty() {
                                            serde_json::Value::Null
                                        } else {
                                            serde_json::Value::String(body_str.to_string())
                                        }
                                    });
                                    req_map.insert("body".into(), body_json);
                                    req_map.insert("raw_body".into(), serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(&body_bytes)));
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
                                     let chunk = ctx.functions[fid].clone();
                                     let bc_len = chunk.bytecode.len();
                                     let mut sub_executor = Executor {
                                        vm: vm.clone(),
                                        ctx: ctx.clone(), 
                                        current_spans: None,
                                        fiber_yielded: false,
                                        hot_counts: vec![0; bc_len],
                                        recording_trace: None,
                                        is_recording: false,
                                        trace_cache: vec![None; bc_len],
                                        http_req: Some(req_mutex_arc.clone()),
                                        http_req_val: Some(req_val),
                                        terminal_raw_enabled: terminal_raw,
                                        hot_counts_pool: Vec::with_capacity(8),
                                        trace_cache_pool: Vec::with_capacity(8),
                                        locals_pool: Vec::with_capacity(8),
                                        trace_revision: 0,
                                     };
                                     
                                     let mut ip = 0;
                                     let mut locals = vec![req_val];
                                     unsafe { req_val.inc_ref(); } 
                                     locals.resize(chunk.max_locals.max(1), Value::from_bool(false));
                                     
                                     loop {
                                        let vm_arc2 = vm.clone();
                                        let mut glbs2 = Some(vm_arc2.globals.write());
                                        let ores = sub_executor.execute_bytecode_inner(
                                            &chunk.bytecode,
                                            &mut ip,
                                            &mut locals,
                                            &vm_arc2,
                                            &mut glbs2,
                                        );

                                        match &ores {
                                            OpResult::Call(target_f, arg, dst) => {
                                                let sub_res = sub_executor._resume_fiber(target_f.clone(), arg.clone(), &vm_arc2, &mut glbs2);
                                                
                                                if let Some(val) = sub_res {
                                                    let should_write = !target_f.read().is_done 
                                                        || val.is_ptr()  
                                                        || val.is_int()  
                                                        || (val.is_bool() && val.as_bool()); 
                                                    
                                                    if should_write {
                                                        let d = *dst as usize;
                                                        if d < locals.len() {
                                                            let old = locals[d];
                                                            if old.is_ptr() { unsafe { old.dec_ref(); } }
                                                            if val.is_ptr() { unsafe { val.inc_ref(); } }
                                                            locals[d] = val;
                                                        }
                                                    } else {
                                                        if val.is_ptr() { unsafe { val.dec_ref(); } }
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }

                                        drop(glbs2);
                                        let handled = req_mutex_arc.lock().is_none();
                                        if handled || matches!(ores, OpResult::Halt | OpResult::Return(_)) {
                                            break;
                                        }
                                        if !matches!(ores, OpResult::Yield(_) | OpResult::Call(_, _, _)) {
                                            break;
                                        }
                                    }

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
                OpResult::Halt
            }
            OpCode::Wait { src } => {
                let val = locals[src as usize];
                let ms = if val.is_int() { val.as_i64() as u64 } else if val.is_float() { val.as_f64() as u64 } else { 0 };
                drop(glbs.take());
                std::thread::sleep(std::time::Duration::from_millis(ms));
                *glbs = Some(vm_arc.globals.write());
                OpResult::Continue
            }
            OpCode::ArrayInit { dst, base, count } => {
                let mut elems = Vec::with_capacity(count as usize);
                let start = base as usize;
                let end = start + count as usize;
                if end > locals.len() {
                    return OpResult::Halt; // Index out of bounds
                }
                for v in &locals[start..end] {
                    let v = *v;
                    unsafe { v.inc_ref(); }
                    elems.push(v);
                }
                let res = Value::from_array(Arc::new(RwLock::new(elems)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::SetInit { dst, base, count } => {
                let mut elements = std::collections::BTreeSet::new();
                let start = base as usize;
                let end = start + count as usize;
                if end > locals.len() {
                    return OpResult::Halt; // Index out of bounds
                }
                for v in &locals[start..end] {
                    let v = *v;
                    unsafe { v.inc_ref(); }
                    elements.insert(v);
                }
                let res = Value::from_set(Arc::new(RwLock::new(SetData { elements, cache: None })));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::MapInit { dst, base, count } => {
                let mut map = Vec::with_capacity(count as usize);
                let start = base as usize;
                let end = start + (count as usize * 2);
                if end > locals.len() {
                    return OpResult::Halt; // Index out of bounds
                }
                for i in (start..end).step_by(2) {
                    let k = locals[i];
                    let v = locals[i + 1];
                    unsafe { k.inc_ref(); v.inc_ref(); }
                    map.push((k, v));
                }
                let res = Value::from_map(Arc::new(RwLock::new(map)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::SetRange { dst, start, end, step, has_step } => {
                let start_val = locals[start as usize].as_i64();
                let end_val = locals[end as usize].as_i64();
                let step_val = if has_step != 0 { locals[step as usize].as_i64().abs().max(1) } else { 1 };
                let mut elements = std::collections::BTreeSet::new();
                if start_val <= end_val {
                    let mut i = start_val;
                    while i <= end_val {
                        elements.insert(Value::from_i64(i));
                        match i.checked_add(step_val) {
                            Some(next) => i = next,
                            None => break,
                        }
                    }
                } else {
                    let mut i = start_val;
                    while i >= end_val {
                        elements.insert(Value::from_i64(i));
                        match i.checked_sub(step_val) {
                            Some(next) => i = next,
                            None => break,
                        }
                    }
                }
                let res = Value::from_set(Arc::new(RwLock::new(SetData { elements, cache: None })));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::SetName { src, name_idx } => {
                let table_val = locals[src as usize];
                if table_val.is_ptr() && (table_val.0 & 0x000F_0000_0000_0000) == TAG_TBL {
                    let name_val = self.ctx.constants[name_idx as usize];
                    let name_str = String::from_utf8_lossy(&name_val.as_string()).to_string();
                    let t_rc = table_val.as_table();
                    let mut t = t_rc.write();
                    t.table_name = name_str;
                }
                OpResult::Continue
            }
            OpCode::Call { dst, func_idx, base, arg_count } => {
                let args = &locals[(base as usize)..(base as usize + arg_count as usize)];
                let target_chunk = self.ctx.functions[func_idx as usize].clone();
                let call_res = self.run_frame_with_guard(target_chunk, args, vm_arc, glbs, func_idx as usize);
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
                OpResult::Continue
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
                        self.check_start_recording(target_ip, 1000);
                        *ip = target_ip;
                        return OpResult::Continue; 
                    }
                }
                OpResult::Continue
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
                    let g_vals = glbs.as_mut().expect("Globals lock lost");
                    if g_idx < g_vals.len() {
                        let g_val = unsafe { g_vals.get_unchecked_mut(g_idx) };
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
                        self.check_start_recording(target_ip, 1000);
                        *ip = target_ip;
                        return OpResult::Continue;
                    }
                } else if l_val.is_int() {
                    let next = l_val.as_i64().wrapping_add(1);
                    *l_val = Value::from_i64(next);
                    let g_idx = g_idx as usize;
                    let g_vals = glbs.as_mut().expect("Globals lock lost");
                    if g_idx < g_vals.len() {
                        let g_val = unsafe { g_vals.get_unchecked_mut(g_idx) };
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
                        self.check_start_recording(target_ip, 1000);
                        *ip = target_ip;
                        return OpResult::Continue;
                    }
                }
                OpResult::Continue
            }
            OpCode::IncLocal { reg } => {
                let val = locals[reg as usize];
                if val.is_int() {
                    locals[reg as usize] = Value::from_i64(val.as_i64().wrapping_add(1));
                }
                OpResult::Continue
            }
            OpCode::LoopNext { reg, limit_reg, target } => {
                let val = locals[reg as usize];
                let limit = locals[limit_reg as usize];
                if val.is_int() && limit.is_int() {
                    let v = val.as_i64().wrapping_add(1);
                    locals[reg as usize] = Value::from_i64(v);
                    if v <= limit.as_i64() {
                        let target_ip = target as usize;
                        self.check_start_recording(target_ip, 1000);
                        *ip = target_ip;
                        return OpResult::Continue;
                    }
                } else {
                    eprintln!("ERROR: LoopNext on non-integers");
                    return OpResult::Halt;
                }
                OpResult::Continue
            }
            OpCode::ArrayLoopNext { idx_reg, size_reg, target } => {
                let idx_val = locals[idx_reg as usize];
                let size_val = locals[size_reg as usize];
                if idx_val.is_int() && size_val.is_int() {
                    let next_idx = idx_val.as_i64().wrapping_add(1);
                    locals[idx_reg as usize] = Value::from_i64(next_idx);
                    if next_idx < size_val.as_i64() {
                        *ip = target as usize;
                        return OpResult::Continue;
                    }
                }
                OpResult::Continue
            }
            OpCode::MethodCall { dst, kind, base, arg_count } => {
                let receiver = locals[base as usize];
                let args: Vec<Value> = locals[(base as usize + 1)..(base as usize + 1 + arg_count as usize)].to_vec();
                let ores = if receiver.is_db() {
                    self.handle_database_method(dst, receiver.as_db(), kind, &args, None, *ip, locals, vm_arc, glbs)
                } else {
                    self.handle_method_call(dst, receiver, kind, &args, None, *ip, locals, vm_arc, glbs)
                };
                ores
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
                OpResult::Continue
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
                OpResult::Continue
            }
            OpCode::EnvGet { dst, src } => {
                let name = locals[src as usize].to_string();
                let res = match std::env::var(&name) {
                    Ok(v) => Value::from_string(Arc::new(v.into_bytes())),
                    Err(_) => Value::from_bool(false),
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::EnvArgs { dst } => {
                let args: Vec<Value> = std::env::args().map(|s| Value::from_string(Arc::new(s.into_bytes()))).collect();
                let res = Value::from_array(Arc::new(RwLock::new(args)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
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
                        Value::from_string(Arc::new(format!("{}{}", a.to_string(), b.to_string()).into_bytes()))
                    }
                    else if b.is_ptr() && (b.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_STR) {
                        Value::from_string(Arc::new(format!("{}{}", a.to_string(), b.to_string()).into_bytes()))
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
                OpResult::Continue
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
                OpResult::Continue
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
                OpResult::Continue
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
                OpResult::Continue
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
                OpResult::Continue
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
                OpResult::Continue
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
                OpResult::Continue
            }
            OpCode::Equal { dst, src1, src2 } => {
                let res = Value::from_bool(locals[src1 as usize] == locals[src2 as usize]);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::NotEqual { dst, src1, src2 } => {
                let res = Value::from_bool(locals[src1 as usize] != locals[src2 as usize]);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::Greater { dst, src1, src2 } => {
                let res = Value::from_bool(locals[src1 as usize] > locals[src2 as usize]);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::Less { dst, src1, src2 } => {
                let v1 = locals[src1 as usize];
                let v2 = locals[src2 as usize];
                let res = if v1.is_int() && v2.is_int() {
                    Value::from_bool(v1.as_i64() < v2.as_i64())
                } else {
                    Value::from_bool(v1 < v2)
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::GreaterEqual { dst, src1, src2 } => {
                let res = Value::from_bool(locals[src1 as usize] >= locals[src2 as usize]);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::LessEqual { dst, src1, src2 } => {
                let res = Value::from_bool(locals[src1 as usize] <= locals[src2 as usize]);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::And { dst, src1, src2 } => {
                let a = locals[src1 as usize].as_bool();
                let b = locals[src2 as usize].as_bool();
                let res = Value::from_bool(a && b);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::Or { dst, src1, src2 } => {
                let a = locals[src1 as usize].as_bool();
                let b = locals[src2 as usize].as_bool();
                let res = Value::from_bool(a || b);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::Not { dst, src } => {
                let res = Value::from_bool(!locals[src as usize].as_bool());
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::Print { src } => {
                println!("{}", locals[src as usize].to_string());
                OpResult::Continue
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
                OpResult::Continue
            }
            OpCode::IntConcat { dst, src1, src2 } => {
                let a = locals[src1 as usize];
                let b = locals[src2 as usize];
                let s = format!("{}{}", a.to_string(), b.to_string());
                let res = if let Ok(i) = s.parse::<i64>() { Value::from_i64(i) } else { Value::from_string(Arc::new(s.into_bytes())) };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::IncVar { idx } => {
                let idx = idx as usize;
                let g_vals = glbs.as_mut().expect("Globals lock lost");
                if idx < g_vals.len() {
                    let val = unsafe { g_vals.get_unchecked_mut(idx) };
                    if (val.0 & 0xFFFF_0000_0000_0000) == (QNAN_BASE | TAG_INT) {
                        let next = (val.0 & 0x0000_FFFF_FFFF_FFFF).wrapping_add(1) & 0x0000_FFFF_FFFF_FFFF;
                        val.0 = (val.0 & 0xFFFF_0000_0000_0000) | next;
                    } else {
                        if val.is_ptr() { unsafe { val.dec_ref(); } }
                        *val = Value::from_i64(val.as_i64().wrapping_add(1));
                    }
                }
                OpResult::Continue
            }
            OpCode::RandomInt { dst, min, max, step, has_step } => {
                let start = locals[min as usize].as_i64();
                let end = locals[max as usize].as_i64();
                let step_val = if locals[has_step as usize].as_bool() { locals[step as usize].as_i64() } else { 1 };
                let mut rng = rand::rng();
                let diff = end - start;
                let abs_diff = diff.abs();
                let abs_step = step_val.abs().max(1);
                    let steps = abs_diff / abs_step;
                let k = rng.random_range(0..=steps);
                let sign = if diff >= 0 { 1 } else { -1 };
                let res = Value::from_i64(start + k * sign * abs_step);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::RandomFloat { dst, min, max, step, has_step } => {
                let start = locals[min as usize].as_f64();
                let end = locals[max as usize].as_f64();
                let step_val = if locals[has_step as usize].as_bool() {
                    let s = locals[step as usize];
                    if s.is_float() { s.as_f64() } else { s.as_i64() as f64 }
                } else { 0.5 };
                let mut rng = rand::rng();
                let diff = end - start;
                let abs_diff = diff.abs();
                let abs_step = step_val.abs();
                if abs_step > 0.0 {
                    let steps = (abs_diff / abs_step).floor() as i64;
                    let k = rng.random_range(0..=steps);
                    let sign = if diff >= 0.0 { 1.0 } else { -1.0 };
                    let res = Value::from_f64(start + (k as f64) * sign * abs_step);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                } else {
                    use rand::Rng;
                    let t: f64 = rng.random();
                    let res = Value::from_f64(start + t * diff);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                OpResult::Continue
            }
            OpCode::CastInt { dst, src } => {
                let val = locals[src as usize];
                let res = if val.is_int() { val } else if val.is_float() { Value::from_i64(val.as_f64() as i64) } else { Value::from_i64(0) };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::CastFloat { dst, src } => {
                let val = locals[src as usize];
                let res = if val.is_float() { val } else if val.is_int() { Value::from_f64(val.as_i64() as f64) } else { Value::from_f64(0.0) };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::CastString { dst, src } => {
                let res = Value::from_string(Arc::new(locals[src as usize].to_string().into_bytes()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            OpCode::CastBool { dst, src } => {
                let val = locals[src as usize];
                let res = Value::from_bool(val.as_bool());
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            _ => OpResult::Continue,
        }
    }

    fn execute_bytecode_inner<'a>(
        &mut self,
        bytecode: &[OpCode],
        ip: &mut usize,
        locals: &mut [Value],
        vm_arc: &'a Arc<VM>,
        glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>,
    ) -> OpResult {
        while *ip < bytecode.len() {
            if SHUTDOWN.load(Ordering::Relaxed) { return OpResult::Halt; }
            let current_ip = *ip;

            if !self.is_recording && current_ip < self.trace_cache.len() {
                if let Some(trace) = &self.trace_cache[current_ip] {
                    let jit_res = self.execute_trace(trace, ip, locals, glbs.as_mut().unwrap());
                    if let Some(res) = jit_res {
                        return res;
                    }
                    continue;
                }
            }

            if self.recording_trace.is_some() {
                self.process_finished_trace(current_ip);
            }

            let op = bytecode[current_ip];
            *ip += 1;

            if self.is_recording {
                self.record_op(op, current_ip, locals);
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
                    let val = glbs.as_ref().unwrap().get(idx as usize).cloned().unwrap_or(Value::from_bool(false));
                    if val.is_ptr() { unsafe { val.inc_ref(); } }
                    let d = &mut locals[dst as usize];
                    if d.is_ptr() { unsafe { d.dec_ref(); } }
                    *d = val;
                }
                OpCode::SetVar { idx, src } => {
                    let val = locals[src as usize];
                    if val.is_ptr() { unsafe { val.inc_ref(); } }
                    let idx = idx as usize;
                    let g_vals = glbs.as_mut().unwrap();
                    if idx >= g_vals.len() { g_vals.resize(idx + 1, Value::from_bool(false)); }
                    let g = &mut g_vals[idx];
                    if g.is_ptr() { unsafe { g.dec_ref(); } }
                    *g = val;
                }
                OpCode::Jump { target } => { 
                    let target_ip = target as usize;
                    if target_ip < current_ip {
                        self.check_start_recording(target_ip, 50);
                    }
                    *ip = target_ip; 
                    continue; 
                }
                OpCode::JumpIfFalse { src, target } => {
                    let val = locals[src as usize];
                    let ok = if val.is_bool() { val.as_bool() } else { val.is_ptr() };
                    if !ok { *ip = target as usize; continue; }
                }
                OpCode::JumpIfTrue { src, target } => {
                    let val = locals[src as usize];
                    let ok = if val.is_bool() { val.as_bool() } else { val.is_ptr() };
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
                _ => {
                    let res = self.execute_bytecode_extended(op, ip, locals, vm_arc, glbs);
                    match res {
                        OpResult::Continue => {}
                        _ => return res,
                    }
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

    fn native_inject_table(&mut self, table_val: &Value, native_json: Value, mapping_val: &Value) {
        if !table_val.is_ptr() || (table_val.0 & 0x000F_0000_0000_0000) != TAG_TBL { return; }
        if !mapping_val.is_ptr() || (mapping_val.0 & 0x000F_0000_0000_0000) != TAG_MAP { return; }

        let table_rc = table_val.as_table();
        let mut table = table_rc.write();
        let mapping_rc = mapping_val.as_map();
        let mapping = mapping_rc.read();

        let items = if native_json.is_array() {
            let arr_rc = native_json.as_array();
            arr_rc.read().clone()
        } else {
            vec![native_json]
        };

        for item in items {
            let mut new_row = Vec::with_capacity(table.columns.len());
            for col in &table.columns {
                let mut found = false;
                for (k, v) in mapping.iter() {
                    if k.is_string() && v.is_string() {
                        let col_name = k.to_string();
                        let json_path = v.to_string();
                        if col_name == col.name {
                            let val = get_path_value_xcx(item, &json_path);
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

    fn handle_array_method<'a>(&mut self, dst: u8, arr_rc: Arc<RwLock<Vec<Value>>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
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
                if args[0].is_int() {
                    let arr = arr_rc.read();
                    let i = args[0].as_i64();
                    if i >= 0 && (i as usize) < arr.len() {
                        let v = arr[i as usize];
                        unsafe { v.inc_ref(); }
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = v;
                    } else {
                        eprintln!("R303: Array index out of bounds: {} (Array length: {}){}", i, arr.len(), self.current_span_info(ip));
                        self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        return OpResult::Halt;
                    }
                } else if args[0].is_string() {
                    let path = args[0].to_string();
                    if path.starts_with('/') {
                        let v = get_path_value_xcx(Value::from_array(arr_rc.clone()), &path);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = v;
                    } else {
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = Value::from_bool(false);
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
                        self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        return OpResult::Halt;
                    }
                } else { 
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Update | MethodKind::Set => {
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
                    } else if i >= 0 {
                        // Auto-resize for XCX dynamic array behavior
                        arr.resize((i as usize) + 1, Value::from_bool(false));
                        unsafe { val.inc_ref(); }
                        arr[i as usize] = val;
                        let res = Value::from_bool(true);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    } else {
                        eprintln!("R303: Array update index out of bounds: {}{}", i, self.current_span_info(ip));
                        self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        return OpResult::Halt;
                    }
                } else if args[0].is_string() {
                    let path = args[0].to_string();
                    if path.starts_with('/') {
                        let val = args[1];
                        set_path_value_xcx(Value::from_array(arr_rc.clone()), &path, val);
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
                        self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
                } else { b"".to_vec() };
                let arr = arr_rc.read();
                let res_bytes = arr.iter()
                    .map(|v| {
                        if v.is_string() { (*v.as_string()).clone() }
                        else { v.to_string().into_bytes() }
                    })
                    .collect::<Vec<_>>()
                    .join(sep.as_slice());
                let res = Value::from_string(Arc::new(res_bytes));
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
            MethodKind::ToStr => {
                let arr_val = Value::from_array(arr_rc.clone());
                let s = arr_val.to_string();
                unsafe { arr_val.dec_ref(); }
                let res = Value::from_string(Arc::new(s.into_bytes()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::ToJson => {
                let arr_val = Value::from_array(arr_rc.clone());
                let json = value_to_json(&arr_val);
                unsafe { arr_val.dec_ref(); }
                let res = Value::from_json(Arc::new(RwLock::new(json)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => { eprintln!("Method {:?} not supported for Array{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_set_method<'a>(&mut self, dst: u8, set_rc: Arc<RwLock<SetData>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
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

    fn handle_string_method<'a>(&mut self, dst: u8, s: &[u8], kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        match kind {
            MethodKind::Length | MethodKind::Size => {
                let res = Value::from_i64(s.len() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Upper  => {
                let s_str = String::from_utf8_lossy(s);
                let res = Value::from_string(Arc::new(s_str.to_uppercase().into_bytes()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Lower  => {
                let s_str = String::from_utf8_lossy(s);
                let res = Value::from_string(Arc::new(s_str.to_lowercase().into_bytes()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Trim   => {
                let s_str = String::from_utf8_lossy(s);
                let res = Value::from_string(Arc::new(s_str.trim().to_string().into_bytes()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::IndexOf => {
                let s_lossy = String::from_utf8_lossy(&s);
                let res = if let Some(v) = args.first() {
                    if v.is_ptr() && (v.0 & 0x000F_0000_0000_0000) == TAG_STR {
                        let sub_bytes = v.as_string();
                        let sub = String::from_utf8_lossy(&sub_bytes);
                        let idx = s_lossy.find(sub.as_ref()).map(|i| i as i64).unwrap_or(-1);
                        Value::from_i64(idx)
                    } else { Value::from_i64(-1) }
                } else { Value::from_i64(-1) };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::LastIndexOf => {
                let s_lossy = String::from_utf8_lossy(&s);
                let res = if let Some(v) = args.first() {
                    if v.is_ptr() && (v.0 & 0x000F_0000_0000_0000) == TAG_STR {
                        let sub_bytes = v.as_string();
                        let sub = String::from_utf8_lossy(&sub_bytes);
                        let idx = s_lossy.rfind(sub.as_ref()).map(|i| i as i64).unwrap_or(-1);
                        Value::from_i64(idx)
                    } else { Value::from_i64(-1) }
                } else { Value::from_i64(-1) };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Replace => {
                if args.len() != 2 { return OpResult::Halt; }
                let from = args[0].as_string();
                let to   = args[1].as_string();
                if from.is_empty() { 
                    eprintln!("R307: .replace() called with empty 'from'{}", self.current_span_info(ip)); 
                    return OpResult::Halt; 
                }
                // Naive byte-level replace for simplicity, or convert to string if possible.
                // Given we are in TAG_STR handle, s is Vec<u8>.
                let s_str = String::from_utf8_lossy(&s);
                let from_str = String::from_utf8_lossy(&from);
                let to_str = String::from_utf8_lossy(&to);
                let res_str = s_str.replace(from_str.as_ref(), to_str.as_ref());
                let res = Value::from_string(Arc::new(res_str.into_bytes()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Slice => {
                if args.len() != 2 { return OpResult::Halt; }
                if !args[0].is_int() || !args[1].is_int() { return OpResult::Halt; }
                let start = args[0].as_i64();
                let end   = args[1].as_i64();
                
                // For strings, usually we want character-aware slicing.
                let s_str = String::from_utf8_lossy(&s);
                let chars: Vec<char> = s_str.chars().collect();
                let len = chars.len() as i64;
                if start < 0 || end > len || start > end {
                    eprintln!("R303: String.slice out of bounds [{}, {}] for len {}{}", start, end, len, self.current_span_info(ip));
                    self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    return OpResult::Halt;
                }
                let res_str: String = chars[start as usize..end as usize].iter().collect();
                let res = Value::from_string(Arc::new(res_str.into_bytes()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Split => {
                if args.is_empty() { return OpResult::Halt; }
                let sep_bytes = args[0].as_string();
                let s_str = String::from_utf8_lossy(&s);
                let sep = String::from_utf8_lossy(&sep_bytes);
                let parts: Vec<Value> = s_str.split(sep.as_ref()).map(|p| Value::from_string(Arc::new(p.to_string().into_bytes()))).collect();
                let res = Value::from_array(Arc::new(RwLock::new(parts)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::StartsWith => {
                if args.is_empty() { return OpResult::Halt; }
                let prefix = args[0].as_string();
                let res = Value::from_bool(s.starts_with(&prefix));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::EndsWith => {
                if args.is_empty() { return OpResult::Halt; }
                let suffix = args[0].as_string();
                let res = Value::from_bool(s.ends_with(&suffix));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::ToInt => {
                let s_str = String::from_utf8_lossy(&s);
                match s_str.trim().parse::<i64>() {
                    Ok(n) => {
                        let res = Value::from_i64(n);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    }
                    Err(_) => {
                        eprintln!("halt.error: Cannot convert \"{}\" to Integer{}", s_str, self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                }
            }
            MethodKind::ToFloat => {
                let s_str = String::from_utf8_lossy(&s);
                match s_str.trim().parse::<f64>() {
                    Ok(f) => {
                        let res = Value::from_f64(f);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    }
                    Err(_) => {
                        eprintln!("halt.error: Cannot convert \"{}\" to Float{}", s_str, self.current_span_info(ip));
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

    fn handle_map_method<'a>(&mut self, dst: u8, map_rc: Arc<RwLock<Vec<(Value, Value)>>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        match kind {
            MethodKind::Get => {
                let key = &args[0];
                let map = map_rc.read();
                if let Some((_, v)) = map.iter().find(|(k, _)| k == key) {
                    unsafe { v.inc_ref(); }
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = *v;
                } else if key.is_string() {
                    let path = key.to_string();
                    if path.starts_with('/') || path.contains('[') || path.contains('.') {
                        drop(map);
                        let v = get_path_value_xcx(Value::from_map(map_rc.clone()), &path);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = v;
                    } else {
                        eprintln!("R304: Map key not found: {}{}", key.to_string(), self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else {
                    eprintln!("R304: Map key not found: {}{}", key.to_string(), self.current_span_info(ip));
                    return OpResult::Halt;
                }
            }
            MethodKind::Set | MethodKind::Insert => {
                let key = args[0]; 
                let val = args[1];
                if key.is_string() {
                    let path = key.to_string();
                    if path.starts_with('/') || path.contains('[') || path.contains('.') {
                        set_path_value_xcx(Value::from_map(map_rc.clone()), &path, val);
                        let res = Value::from_bool(true);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                        return OpResult::Continue;
                    }
                }
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
            MethodKind::Exists => {
                let key = &args[0];
                let found = if key.is_string() {
                    let path = key.to_string();
                    if path.starts_with('/') {
                        let v = get_path_value_xcx(Value::from_map(map_rc.clone()), &path);
                        let exists = v.is_ptr() || v.is_bool() || v.is_int() || v.is_float();
                        unsafe { v.dec_ref(); }
                        exists && !v.is_bool_false()
                    } else {
                        map_rc.read().iter().any(|(k, _)| k == key)
                    }
                } else {
                    map_rc.read().iter().any(|(k, _)| k == key)
                };
                let res = Value::from_bool(found);
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
            MethodKind::Inject => {
                let ok = if args.len() == 2 {
                    if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_MAP &&
                       args[1].is_ptr() && (args[1].0 & 0x000F_0000_0000_0000) == TAG_TBL {
                        self.native_inject_table(&args[1], Value::from_map(map_rc.clone()), &args[0]);
                        true
                    } else { false }
                } else if args.len() == 3 {
                    if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_STR &&
                       args[1].is_ptr() && (args[1].0 & 0x000F_0000_0000_0000) == TAG_MAP &&
                       args[2].is_ptr() && (args[2].0 & 0x000F_0000_0000_0000) == TAG_TBL {
                        let path = args[0].to_string();
                        let sub_val = get_path_value_xcx(Value::from_map(map_rc.clone()), &path);
                        self.native_inject_table(&args[2], sub_val, &args[1]);
                        unsafe { sub_val.dec_ref(); }
                        true
                    } else { false }
                } else { false };
                let res = Value::from_bool(ok);
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
                let key = &args[0];
                let has = map_rc.read().iter().any(|(k, _)| k == key);
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
            MethodKind::ToJson => {
                let map_val = Value::from_map(map_rc.clone());
                let json = value_to_json(&map_val);
                unsafe { map_val.dec_ref(); }
                let res = Value::from_json(Arc::new(RwLock::new(json)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::ToStr => {
                let map_val = Value::from_map(map_rc.clone());
                let s = map_val.to_string();
                unsafe { map_val.dec_ref(); }
                let res = Value::from_string(Arc::new(s.into_bytes()));
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

    fn handle_table_method<'a>(&mut self, dst: u8, t_rc: Arc<RwLock<TableData>>, kind: MethodKind, args: &[Value], names: Option<&[String]>, ip: usize, locals: &mut [Value], vm_arc: &'a Arc<VM>, glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        let t = t_rc.read();
        match kind {
            MethodKind::Count | MethodKind::Len | MethodKind::Size => {
                let res = Value::from_i64(t.rows.len() as i64);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Show => {
                println!("{}", t.to_formatted_grid());
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Insert | MethodKind::Add | MethodKind::Save => {
                drop(t);
                let mut t_mut = t_rc.write();
                let cols = t_mut.columns.clone();
                let mut row = Vec::with_capacity(cols.len());
                
                let mut pk_val = None;
                let mut pk_idx = None;

                let mut mapped_row = vec![None; cols.len()];
                let mut pos_vals = Vec::new();

                if let Some(ns) = names {
                    for (i, n) in ns.iter().enumerate() {
                        if n.is_empty() {
                            pos_vals.push(args[i]);
                        } else {
                            if let Some(ci) = cols.iter().position(|c| &c.name == n) {
                                mapped_row[ci] = Some(args[i]);
                            }
                        }
                    }
                } else {
                    pos_vals.extend_from_slice(args);
                }

                let mut pos_idx = 0;
                for (ci, col) in cols.iter().enumerate() {
                    if col.is_auto {
                        let max = t_mut.rows.iter()
                            .filter_map(|r| if r[ci].is_int() { Some(r[ci].as_i64()) } else { None })
                            .max().unwrap_or(0);
                        row.push(Value::from_i64(max + 1));
                    } else {
                        let val = if let Some(v) = mapped_row[ci] {
                            v
                        } else if pos_idx < pos_vals.len() {
                            let v = pos_vals[pos_idx];
                            pos_idx += 1;
                            v
                        } else {
                            Value::from_bool(false)
                        };
                        
                        unsafe { val.inc_ref(); }
                        row.push(val);

                        if col.is_pk {
                            pk_val = Some(val);
                            pk_idx = Some(ci);
                        }
                    }
                }

                let mut replaced = false;
                if kind == MethodKind::Save && pk_idx.is_some() {
                    let pki = pk_idx.unwrap();
                    let pkv = pk_val.unwrap();
                    if let Some(existing_idx) = t_mut.rows.iter().position(|r| r[pki] == pkv) {
                        let old_row = std::mem::replace(&mut t_mut.rows[existing_idx], row.clone());
                        for v in old_row { unsafe { v.dec_ref(); } }
                        replaced = true;
                        
                        if let Some(binding) = &t_mut.sql_binding {
                            let mut sql = format!("UPDATE [{}] SET ", binding.table_name);
                            let mut first = true;
                            let mut pieces = Vec::new();
                            for (ci, col) in cols.iter().enumerate() {
                                if ci == pki { continue; }
                                if !first { sql.push_str(", "); }
                                first = false;
                                sql.push_str(&format!("[{}] = ?", col.name));
                                pieces.push(row[ci]);
                            }
                            sql.push_str(&format!(" WHERE [{}] = ?", cols[pki].name));
                            pieces.push(row[pki]);
                            
                            let conn = binding.db_conn.lock();
                            match conn.prepare(&sql) {
                                Ok(mut stmt) => {
                                    for (i, v) in pieces.iter().enumerate() {
                                        if v.is_int() { let _ = stmt.raw_bind_parameter(i + 1, v.as_i64()); }
                                        else if v.is_float() { let _ = stmt.raw_bind_parameter(i + 1, v.as_f64()); }
                                        else if v.is_bool() { let _ = stmt.raw_bind_parameter(i + 1, if v.as_bool() { 1 } else { 0 }); }
                                        else { let _ = stmt.raw_bind_parameter(i + 1, v.to_string()); }
                                    }
                                    if let Err(e) = stmt.raw_execute() {
                                        eprintln!("R402: SQL update error: {}{}", e, self.current_span_info(ip));
                                    }
                                }
                                Err(e) => {
                                    eprintln!("R403: SQL update prepare error: {}{}", e, self.current_span_info(ip));
                                }
                            }
                        }
                    }
                }

                if !replaced {
                    t_mut.rows.push(row.clone());
                    if let Some(binding) = &t_mut.sql_binding {
                        let mut sql = format!("INSERT INTO [{}] (", binding.table_name);
                        let mut vals_sql = String::from("VALUES (");
                        let mut pieces = Vec::new();
                        let mut first = true;
                        for (ci, col) in cols.iter().enumerate() {
                            if !first { sql.push_str(", "); vals_sql.push_str(", "); }
                            first = false;
                            sql.push_str(&format!("[{}]", col.name));
                            vals_sql.push('?');
                            pieces.push(row[ci]);
                        }
                        sql.push_str(") ");
                        sql.push_str(&vals_sql);
                        sql.push(')');
                        
                        let conn = binding.db_conn.lock();
                        match conn.prepare(&sql) {
                            Ok(mut stmt) => {
                                for (i, v) in pieces.iter().enumerate() {
                                    if v.is_int() { let _ = stmt.raw_bind_parameter(i + 1, v.as_i64()); }
                                    else if v.is_float() { let _ = stmt.raw_bind_parameter(i + 1, v.as_f64()); }
                                    else if v.is_bool() { let _ = stmt.raw_bind_parameter(i + 1, if v.as_bool() { 1 } else { 0 }); }
                                    else { let _ = stmt.raw_bind_parameter(i + 1, v.to_string()); }
                                }
                                if let Err(e) = stmt.raw_execute() {
                                    eprintln!("R402: SQL insert error: {}{}", e, self.current_span_info(ip));
                                }
                            }
                            Err(e) => {
                                eprintln!("R403: SQL insert prepare error: {}{}", e, self.current_span_info(ip));
                            }
                        }
                    }
                }

                let affected = 1;
                let mut insert_id = 0;
                if let Some(binding) = &t_mut.sql_binding {
                    let conn = binding.db_conn.lock();
                    insert_id = conn.last_insert_rowid();
                }

                let mut obj = serde_json::Map::new();
                obj.insert("affected".to_string(), serde_json::Value::Number(affected.into()));
                obj.insert("insertId".to_string(), serde_json::Value::Number(insert_id.into()));
                let res = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(obj))));
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
                            let mut obj = serde_json::Map::new();
                            obj.insert("affected".to_string(), serde_json::Value::Number(1.into()));
                            obj.insert("insertId".to_string(), serde_json::Value::Number(0.into()));
                            let res = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(obj))));
                            unsafe { locals[dst as usize].dec_ref(); }
                            locals[dst as usize] = res;
                        } else { 
                            let mut obj = serde_json::Map::new();
                            obj.insert("affected".to_string(), serde_json::Value::Number(0.into()));
                            obj.insert("insertId".to_string(), serde_json::Value::Number(0.into()));
                            let res = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(obj))));
                            unsafe { locals[dst as usize].dec_ref(); }
                            locals[dst as usize] = res;
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
                        let mut obj = serde_json::Map::new();
                        obj.insert("affected".to_string(), serde_json::Value::Number(1.into()));
                        obj.insert("insertId".to_string(), serde_json::Value::Number(0.into()));
                        let res = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(obj))));
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    } else { 
                        let mut obj = serde_json::Map::new();
                        obj.insert("affected".to_string(), serde_json::Value::Number(0.into()));
                        obj.insert("insertId".to_string(), serde_json::Value::Number(0.into()));
                        let res = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(obj))));
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    }
                } else { 
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Where => {
                let (filter_func, captures) = if args[0].is_func() {
                    (args[0].as_function(), vec![])
                } else if args[0].is_fiber() {
                    let fib = args[0].as_fiber();
                    let f_read = fib.read();
                    (f_read.func_id as u32, f_read.locals.clone())
                } else {
                    eprintln!("R301: Table.where() requires a function or fiber. Got: {:x}{}", args[0].0, self.current_span_info(ip));
                    return OpResult::Halt;
                };

                let row_count = t.rows.len();
                let sql_where = if t.sql_binding.is_some() {
                    self.translate_filter_to_sql(filter_func as usize, &t.columns, &captures)
                } else { None };

                if let Some(MethodKind::Remove) = t.pending_op {
                    if let Some(binding) = &t.sql_binding {
                        let conn = binding.db_conn.lock();
                        let mut sql = format!("DELETE FROM [{}]", binding.table_name);
                        if let Some(w) = &sql_where {
                            sql.push_str(" WHERE ");
                            sql.push_str(w);
                        }
                        
                        let res = match conn.execute(&sql, []) {
                            Ok(affected) => {
                                let mut obj = serde_json::Map::new();
                                obj.insert("affected".to_string(), serde_json::Value::Number(affected.into()));
                                obj.insert("insertId".to_string(), serde_json::Value::Number(0.into()));
                                Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(obj))))
                            }
                            Err(e) => {
                                eprintln!("R403: Delete error: {}{}", e, self.current_span_info(ip));
                                Value::from_bool(false)
                            }
                        };
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                        return OpResult::Continue;
                    }
                }

                drop(t);
                let mut filtered = Vec::new();
                for i in 0..row_count {
                    let row_ref = Arc::new(RowRef { table: t_rc.clone(), row_idx: i as u32 });
                    let row_val = Value::from_row(row_ref);
                    let mut run_args = vec![row_val];
                    for a in &args[1..] { unsafe { a.inc_ref(); } run_args.push(*a); }
                    if let Some(res) = self.run_frame_with_guard(self.ctx.functions[filter_func as usize].clone(), &run_args, vm_arc, glbs, filter_func as usize) {
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
                
                let t_read = t_rc.read();
                let res = Value::from_table(Arc::new(RwLock::new(
                    TableData {
                        table_name: t_read.table_name.clone(), 
                        columns: t_read.columns.clone(), 
                        rows: filtered, 
                        sql_binding: t_read.sql_binding.clone(),
                        sql_where,
                        pending_op: None,
                    }
                )));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Find => {
                if args.is_empty() { 
                    eprintln!("Table.find: missing predicate argument{}", self.current_span_info(ip));
                    return OpResult::Halt; 
                }
                let filter_func = if args[0].is_func() {
                    args[0].as_function()
                } else if args[0].is_fiber() {
                    args[0].as_fiber().read().func_id as u32
                } else {
                    eprintln!("Table.find: first argument must be a function or fiber{}", self.current_span_info(ip));
                    return OpResult::Halt;
                };
                let row_count = t.rows.len();
                drop(t);
                let mut found_idx: i64 = -1;
                for i in 0..row_count {
                    let row_ref = Arc::new(RowRef { table: t_rc.clone(), row_idx: i as u32 });
                    let row_val = Value::from_row(row_ref);
                    let mut run_args = vec![row_val];
                    for a in &args[1..] { unsafe { a.inc_ref(); } run_args.push(*a); }
                    if let Some(res) = self.run_frame_with_guard(self.ctx.functions[filter_func as usize].clone(), &run_args, vm_arc, glbs, filter_func as usize) {
                        if res.is_bool() && res.as_bool() {
                            found_idx = i as i64;
                            unsafe { res.dec_ref(); }
                            unsafe { row_val.dec_ref(); }
                            for a in run_args.into_iter().skip(1) { unsafe { a.dec_ref(); } }
                            break;
                        }
                        unsafe { res.dec_ref(); }
                    }
                    unsafe { row_val.dec_ref(); }
                    for a in run_args.into_iter().skip(1) { unsafe { a.dec_ref(); } }
                }
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = Value::from_i64(found_idx);
            }
            MethodKind::Get => {
                let idx = if args[0].is_int() { args[0].as_i64() } else { -1 };
                if idx >= 0 && (idx as usize) < t.rows.len() {
                    let res = Value::from_row(Arc::new(RowRef { table: t_rc.clone(), row_idx: idx as u32 }));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                } else {
                    eprintln!("R303: Table.get index out of bounds: {}{}", idx, self.current_span_info(ip));
                    self.vm.error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
                        let left_key_bytes = args[1].as_string();
                        let right_key_bytes = args[2].as_string();
                        JoinPred::Keys(String::from_utf8_lossy(&left_key_bytes).into_owned(), String::from_utf8_lossy(&right_key_bytes).into_owned())
                    } else {
                        eprintln!("join: key args must be strings{}", self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else if args.len() == 2 {
                    if args[1].is_ptr() && (args[1].0 & 0x000F_0000_0000_0000) == TAG_FUNC {
                        JoinPred::Lambda(args[1].as_function() as usize)
                    } else if args[1].is_fiber() {
                        let fib = args[1].as_fiber();
                        let f_read = fib.read();
                        JoinPred::Closure(f_read.func_id as usize, f_read.locals.clone())
                    } else {
                        eprintln!("join: second arg must be a function or closure{}", self.current_span_info(ip));
                        return OpResult::Halt;
                    }
                } else {
                    eprintln!("join: requires 2 or 3 arguments{}", self.current_span_info(ip));
                    return OpResult::Halt;
                };
                let left_data  = t.clone();
                let right_data = right_rc.read().clone();
                drop(t);
                let result = join_tables(&left_data, &right_data, &pred, "b", self, vm_arc, glbs);
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
            MethodKind::ToJson => {
                let json = t.to_json();
                drop(t);
                let res = Value::from_json(Arc::new(RwLock::new(json)));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Fetch | MethodKind::Query => {
                let sql = if kind == MethodKind::Query && !args.is_empty() {
                    args[0].to_string()
                } else {
                    let table_name = if let Some(binding) = &t.sql_binding {
                        binding.table_name.clone()
                    } else { "unknown".to_string() };
                    let mut s = format!("SELECT * FROM [{}]", table_name);
                    if let Some(w) = &t.sql_where {
                        s.push_str(" WHERE ");
                        s.push_str(w);
                    }
                    s
                };
                
                let binding_opt = t.sql_binding.clone();
                let cols = t.columns.clone();

                if let Some(binding) = binding_opt {
                    let mut new_rows = Vec::new();
                    let mut ok = false;
                    {
                        let conn = binding.db_conn.lock();
                        if let Ok(mut stmt) = conn.prepare(&sql) {
                            if let Ok(rows_iter) = stmt.query_map([], |row| {
                                let mut xcx_row = Vec::new();
                                for i in 0..cols.len() {
                                    let col = &cols[i];
                                    let val = match col.ty {
                                        crate::parser::ast::Type::Int => Value::from_i64(row.get(i).unwrap_or(0)),
                                        crate::parser::ast::Type::Float => Value::from_f64(row.get(i).unwrap_or(0.0)),
                                        crate::parser::ast::Type::Bool => Value::from_bool(row.get::<_, i32>(i).unwrap_or(0) != 0),
                                        _ => Value::from_string(Arc::new(row.get::<_, String>(i).unwrap_or_default().into_bytes())),
                                    };
                                    xcx_row.push(val);
                                }
                                Ok(xcx_row)
                            }) {
                                for r in rows_iter { if let Ok(row) = r { new_rows.push(row); } }
                                ok = true;
                            }
                        }
                    }

                    if ok {
                        drop(t);
                        let res = Value::from_table(Arc::new(RwLock::new(TableData {
                            table_name: String::new(),
                            columns: cols,
                            rows: new_rows,
                            sql_binding: Some(binding),
                            sql_where: None,
                            pending_op: None,
                        })));
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = res;
                    } else {
                        drop(t);
                        unsafe { locals[dst as usize].dec_ref(); }
                        locals[dst as usize] = Value::from_bool(false);
                    }
                } else {
                    let rows_copy = t.rows.iter().map(|r| {
                        r.iter().map(|v| { unsafe { v.inc_ref(); } *v }).collect()
                    }).collect();
                    drop(t);
                    let res = Value::from_table(Arc::new(RwLock::new(TableData {
                        table_name: String::new(),
                        columns: cols,
                        rows: rows_copy,
                        sql_binding: None,
                        sql_where: None,
                        pending_op: None,
                    })));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
            }
            MethodKind::First => {
                let res = if let Some(_first_row) = t.rows.first() {
                    Value::from_row(Arc::new(RowRef {
                        table: t_rc.clone(),
                        row_idx: 0,
                    }))
                } else {
                    Value::from_bool(false)
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => { eprintln!("Method {:?} not supported for Table{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    fn handle_row_method<'a>(&mut self, dst: u8, row_ref: Arc<RowRef>, kind: MethodKind, ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
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

    fn handle_row_custom<'a>(&mut self, dst: u8, row_ref: Arc<RowRef>, method_name: &[u8], ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        let t = row_ref.table.read();
        let m_name_str = String::from_utf8_lossy(method_name);
        
        if let Some(col_idx) = t.columns.iter().position(|c| c.name == m_name_str) {
            let row_idx = row_ref.row_idx as usize;
            
            let v = t.rows[row_idx][col_idx];
            unsafe { v.inc_ref(); }
            
            let old = locals[dst as usize];
            if old.is_ptr() { unsafe { old.dec_ref(); } }
            
            locals[dst as usize] = v;
        } else {
            match m_name_str.as_ref() {
                "show" => {
                    let row_val = Value::from_row(row_ref.clone());
                    println!("{}", row_val.to_string());
                    unsafe { row_val.dec_ref(); }
                    let res = Value::from_bool(true);
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                }
                _ => { 
                    eprintln!("Unknown Row member: {}{}", m_name_str, self.current_span_info(ip)); 
                    return OpResult::Halt; 
                }
            }
        }
        OpResult::Continue
    }

    fn handle_date_method<'a>(&mut self, dst: u8, d: chrono::NaiveDateTime, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
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
                let fmt_bytes = if let Some(v) = args.first() {
                    if v.is_ptr() && (v.0 & 0x000F_0000_0000_0000) == TAG_STR {
                        v.as_string()
                    } else { Arc::new(b"%Y-%m-%d %H:%M:%S".to_vec()) }
                } else {
                    Arc::new(b"%Y-%m-%d %H:%M:%S".to_vec())
                };
                let fmt_str = String::from_utf8_lossy(&fmt_bytes)
                    .replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d")
                    .replace("HH", "%H").replace("mm", "%M").replace("ss", "%S")
                    .replace("SSS", "%3f").replace("ms", "%3f");
                let res = Value::from_string(Arc::new(d.format(&fmt_str).to_string().into_bytes()));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            _ => { eprintln!("Method {:?} not supported for Date{}", kind, self.current_span_info(ip)); return OpResult::Halt; }
        }
        OpResult::Continue
    }

    #[inline(never)]
    fn handle_database_method<'a>(&mut self, dst: u8, db_rc: Arc<DatabaseData>, kind: MethodKind, args: &[Value], names: Option<&[String]>, ip: usize, locals: &mut [Value], vm_arc: &'a Arc<VM>, glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        match kind {
            MethodKind::Begin => {
                let conn = db_rc.conn.lock();
                let res = conn.execute("BEGIN", []);
                let val = Value::from_bool(res.is_ok());
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = val;
            }
            MethodKind::Commit => {
                let conn = db_rc.conn.lock();
                let res = conn.execute("COMMIT", []);
                let val = Value::from_bool(res.is_ok());
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = val;
            }
            MethodKind::Rollback => {
                let conn = db_rc.conn.lock();
                let res = conn.execute("ROLLBACK", []);
                let val = Value::from_bool(res.is_ok());
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = val;
            }
            MethodKind::Sync | MethodKind::Drop | MethodKind::Truncate => {
                let conn = db_rc.conn.lock();
                if kind == MethodKind::Drop || kind == MethodKind::Truncate {
                    if args.is_empty() { return OpResult::Halt; }
                    let table_val = args[0];
                    if !table_val.is_ptr() || (table_val.0 & 0x000F_0000_0000_0000) != TAG_TBL { return OpResult::Halt; }
                    let t_rc = table_val.as_table();
                    let t = t_rc.read();
                    let table_name = if let Some(binding) = &t.sql_binding { binding.table_name.clone() } else { "unknown".to_string() };
                    let sql = if kind == MethodKind::Drop {
                        format!("DROP TABLE IF EXISTS [{}]", table_name)
                    } else {
                        format!("DELETE FROM [{}]", table_name)
                    };
                    let affected = match conn.execute(&sql, []) {
                        Ok(n) => n,
                        Err(e) => {
                            eprintln!("R402: SQL {:?} error: {}{}", kind, e, self.current_span_info(ip));
                            0
                        }
                    };
                    let mut obj = serde_json::Map::new();
                    obj.insert("affected".to_string(), serde_json::Value::Number(affected.into()));
                    obj.insert("insertId".to_string(), serde_json::Value::Number(0.into()));
                    let val = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(obj))));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = val;
                } else {
                    let mut affected = 0;
                    if !args.is_empty() {
                        let table_val = args[0];
                        if table_val.is_ptr() && (table_val.0 & 0x000F_0000_0000_0000) == TAG_TBL {
                            let t_rc = table_val.as_table();
                            let mut t = t_rc.write();
                            
                            let mut name = t.table_name.clone();
                            if name.is_empty() {
                                {
                                    let tables = db_rc.tables.read();
                                    if let Some((n, _)) = tables.iter().find(|(_, v)| **v == table_val) {
                                        name = n.clone();
                                    }
                                }
                                if name.is_empty() { name = "unknown".to_string(); }
                            }
                                                        
                            t.sql_binding = Some(crate::backend::vm::SqlBinding {
                                db_conn: db_rc.conn.clone(),
                                table_name: name.clone(),
                            });
                            
                            let mut sql = format!("CREATE TABLE IF NOT EXISTS [{}] (", name);
                            let mut first = true;
                            for col in &t.columns {
                                if !first { sql.push_str(", "); }
                                first = false;
                                sql.push_str(&format!("[{}]", col.name));
                                match col.ty {
                                    crate::parser::ast::Type::Int => sql.push_str(" INTEGER"),
                                    crate::parser::ast::Type::Float => sql.push_str(" REAL"),
                                    crate::parser::ast::Type::Bool => sql.push_str(" INTEGER"),
                                    _ => sql.push_str(" TEXT"),
                                }
                                if col.is_auto { sql.push_str(" PRIMARY KEY AUTOINCREMENT"); }
                                else if col.is_pk { sql.push_str(" PRIMARY KEY"); }
                            }
                            sql.push(')');
                            // Syncing table SQL

                            if let Err(e) = conn.execute(&sql, []) {
                                eprintln!("R300: Failed to sync schema for table {}: {}{}", name, e, self.current_span_info(ip));
                            } else {
                                affected = 1;
                            }
                        }
                    } else {
                        let tables = db_rc.tables.read();
                        for (name, table_val) in tables.iter() {
                            let t_rc = table_val.as_table();
                            let t = t_rc.read();
                            let mut sql = format!("CREATE TABLE IF NOT EXISTS [{}] (", name);
                            let mut first = true;
                            for col in &t.columns {
                                if !first { sql.push_str(", "); }
                                first = false;
                                sql.push_str(&format!("[{}]", col.name));
                                match col.ty {
                                    crate::parser::ast::Type::Int => sql.push_str(" INTEGER"),
                                    crate::parser::ast::Type::Float => sql.push_str(" REAL"),
                                    crate::parser::ast::Type::Bool => sql.push_str(" INTEGER"),
                                    _ => sql.push_str(" TEXT"),
                                }
                                if col.is_auto { sql.push_str(" PRIMARY KEY AUTOINCREMENT"); }
                                else if col.is_pk { sql.push_str(" PRIMARY KEY"); }
                            }
                            sql.push(')');
                            if let Ok(_) = conn.execute(&sql, []) {
                                affected += 1;
                            }
                        }
                    }
                    let mut obj = serde_json::Map::new();
                    obj.insert("affected".to_string(), serde_json::Value::Number(affected.into()));
                    obj.insert("insertId".to_string(), serde_json::Value::Number(0.into()));
                    let val = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(obj))));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = val;
                }
            }
            MethodKind::Has => {
                if args.is_empty() { return OpResult::Halt; }
                let table_val = args[0];
                if !table_val.is_ptr() || (table_val.0 & 0x000F_0000_0000_0000) != TAG_TBL { return OpResult::Halt; }
                let t_rc = table_val.as_table();
                let t = t_rc.read();
                let table_name = if let Some(binding) = &t.sql_binding { binding.table_name.clone() } else { "unknown".to_string() };
                let conn = db_rc.conn.lock();
                let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?").unwrap();
                let exists = stmt.exists([&table_name]).unwrap_or(false);
                let val = Value::from_bool(exists);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = val;
            }
            MethodKind::Remove => {
                if args.is_empty() { return OpResult::Halt; }
                let table_val = args[0];
                if !table_val.is_ptr() || (table_val.0 & 0x000F_0000_0000_0000) != TAG_TBL { return OpResult::Halt; }
                let t_orig_rc = table_val.as_table();
                let t_orig = t_orig_rc.read();
                let res = Value::from_table(Arc::new(RwLock::new(TableData {
                    table_name: t_orig.table_name.clone(),
                    columns: t_orig.columns.clone(),
                    rows: Vec::new(),
                    sql_binding: t_orig.sql_binding.clone(),
                    sql_where: None,
                    pending_op: Some(MethodKind::Remove),
                })));
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Fetch | MethodKind::Query => {
                if args.is_empty() { return OpResult::Halt; }
                let table_val = args[0];
                if !table_val.is_ptr() || (table_val.0 & 0x000F_0000_0000_0000) != TAG_TBL { return OpResult::Halt; }
                let t_orig_rc = table_val.as_table();
                let t_orig = t_orig_rc.read();
                let table_name = if let Some(binding) = &t_orig.sql_binding {
                    binding.table_name.clone()
                } else {
                    "unknown".to_string() 
                };
                
                let conn = db_rc.conn.lock();
                let mut params = Vec::new();
                let sql = if kind == MethodKind::Query {
                    if args.len() > 2 {
                        if let Some(arr_rc) = args[2].as_array_opt() {
                            let arr = arr_rc.read();
                            for v in arr.iter() {
                                params.push(v.to_sql_value());
                            }
                        }
                    }
                    args[1].to_string()
                } else {
                    let mut s = format!("SELECT * FROM [{}]", table_name);
                    if let Some(w) = &t_orig.sql_where {
                        s.push_str(" WHERE ");
                        s.push_str(w);
                    }
                    s
                };
                
                if let Ok(mut stmt) = conn.prepare(&sql) {
                    let rows_iter = stmt.query_map(rusqlite::params_from_iter(params), |row| {
                        let xcx_row = (0..t_orig.columns.len()).map(|i| {
                            let col = &t_orig.columns[i];
                            match col.ty {
                                crate::parser::ast::Type::Int => Value::from_i64(row.get(i).unwrap_or(0)),
                                crate::parser::ast::Type::Float => Value::from_f64(row.get(i).unwrap_or(0.0)),
                                crate::parser::ast::Type::Bool => Value::from_bool(row.get::<_, i32>(i).unwrap_or(0) != 0),
                                _ => Value::from_string(Arc::new(row.get::<_, String>(i).unwrap_or_default().into_bytes())),
                            }
                        }).collect();
                        Ok(xcx_row)
                    }).unwrap();
                    
                    let mut new_rows = Vec::new();
                    for r in rows_iter {
                        if let Ok(row) = r { new_rows.push(row); }
                    }
                    
                    let res = Value::from_table(Arc::new(RwLock::new(TableData {
                        table_name: t_orig.table_name.clone(),
                        columns: t_orig.columns.clone(),
                        rows: new_rows,
                        sql_binding: t_orig.sql_binding.clone(),
                        sql_where: None,
                        pending_op: None,
                    })));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                } else {
                    if let Err(e) = conn.prepare(&sql) {
                        eprintln!("R403: SQL prepare error: {}{}", e, self.current_span_info(ip));
                    }
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Insert | MethodKind::Save => {
                if args.is_empty() { return OpResult::Halt; }
                let table_val = args[0];
                if !table_val.is_ptr() || (table_val.0 & 0x000F_0000_0000_0000) != TAG_TBL { return OpResult::Halt; }
                let t_rc = table_val.as_table();
                let sub_args = &args[1..];
                let sub_names = names.and_then(|n| if n.len() > 1 { Some(&n[1..]) } else { None });
                return self.handle_table_method(dst, t_rc, kind, sub_args, sub_names, ip, locals, vm_arc, glbs);
            }
            MethodKind::QueryRaw => {
                let sql = args[0].to_string();
                let conn = db_rc.conn.lock();
                if let Ok(mut stmt) = conn.prepare(&sql) {
                    let col_count = stmt.column_count();
                    let col_names: Vec<String> = (0..col_count).map(|i| stmt.column_name(i).unwrap_or("?").to_string()).collect();
                    let mut results = Vec::new();
                    if let Ok(mut rows) = stmt.query([]) {
                        while let Some(row) = rows.next().unwrap_or(None) {
                            let mut obj = serde_json::Map::new();
                            for i in 0..col_count {
                                let name = col_names[i].clone();
                                let val: rusqlite::types::Value = row.get(i).unwrap_or(rusqlite::types::Value::Null);
                                let json_val = match val {
                                    rusqlite::types::Value::Null => serde_json::Value::Null,
                                    rusqlite::types::Value::Integer(i) => serde_json::Value::Number(i.into()),
                                    rusqlite::types::Value::Real(f) => serde_json::Number::from_f64(f).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
                                    rusqlite::types::Value::Text(t) => serde_json::Value::String(t),
                                    rusqlite::types::Value::Blob(b) => serde_json::Value::String(hex::encode(b)),
                                };
                                obj.insert(name, json_val);
                            }
                            results.push(serde_json::Value::Object(obj));
                        }
                    }
                    let res = Value::from_json(Arc::new(RwLock::new(serde_json::Value::Array(results))));
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = res;
                } else {
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                }
            }
            MethodKind::Exec => {
                let sql = args[0].to_string();
                let conn = db_rc.conn.lock();
                
                let mut params = Vec::new();
                if args.len() > 1 {
                    if let Some(arr_rc) = args[1].as_array_opt() {
                        let arr = arr_rc.read();
                        for v in arr.iter() {
                            params.push(v.to_sql_value());
                        }
                    }
                }
                
                // Executing SQL logic

                let res = match conn.execute(&sql, rusqlite::params_from_iter(params)) {
                    Ok(affected) => {
                        let mut obj = serde_json::Map::new();
                        obj.insert("affected".to_string(), serde_json::Value::Number(affected.into()));
                        obj.insert("insertId".to_string(), serde_json::Value::Number(conn.last_insert_rowid().into()));
                        Value::from_json(Arc::new(RwLock::new(serde_json::Value::Object(obj))))
                    }
                    Err(e) => {
                        eprintln!("R402: Database.exec error: {}{}", e, self.current_span_info(ip));
                        Value::from_bool(false)
                    }
                };
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
            }
            MethodKind::Close => {
                let val = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = val;
            }
            MethodKind::IsOpen => {
                let val = Value::from_bool(true); 
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = val;
            }
            _ => {
                eprintln!("Method {:?} not supported for Database{}", kind, self.current_span_info(ip));
                return OpResult::Halt;
            }
        }
        OpResult::Continue
    }

    fn handle_json_method<'a>(&mut self, dst: u8, j_rc: Arc<RwLock<serde_json::Value>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        let mut j_mut = j_rc.write();
        match kind {
            MethodKind::Set | MethodKind::Insert | MethodKind::Update => {
                if args.len() >= 2 {
                    if args[0].is_ptr() && (args[0].0 & 0x000F_0000_0000_0000) == TAG_STR {
                        let path_bytes = args[0].as_string();
                        let path = String::from_utf8_lossy(&path_bytes);
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
                        let path_bytes = args[0].as_string();
                        let path = String::from_utf8_lossy(&path_bytes);
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
                    let path_bytes = args[0].as_string();
                    let path = String::from_utf8_lossy(&path_bytes);
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
                    let s_bytes = args[0].as_string();
                    String::from_utf8_lossy(&s_bytes).into_owned()
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
                        let key_bytes = args[0].as_string();
                        let key = String::from_utf8_lossy(&key_bytes);
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
                let res = Value::from_string(Arc::new(j_mut.to_string().into_bytes()));
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

    fn handle_json_custom<'a>(&mut self, dst: u8, j_rc: Arc<RwLock<serde_json::Value>>, field_name: &[u8], args: &[Value], _ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>, base: u8) -> OpResult {
        let j = j_rc.read();
        let f_name_str = String::from_utf8_lossy(field_name);
        
        if f_name_str == "bind" && args.len() >= 1 {
            let pp = normalize_json_path(&args[0].to_string());
            if let Some(v) = j.pointer(&pp) {
                let res = json_serde_to_value(v);
                // Bind to the register of the second argument (if provided)
                if args.len() >= 2 {
                    let target_reg = (base + 2) as usize; 
                    if target_reg < locals.len() {
                        let old = locals[target_reg];
                        if old.is_ptr() { unsafe { old.dec_ref(); } }
                        if res.is_ptr() { unsafe { res.inc_ref(); } }
                        locals[target_reg] = res;
                    }
                }
                
                let ok = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = ok;
                return OpResult::Continue;
            } else {
                let ok = Value::from_bool(false);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = ok;
                return OpResult::Continue;
            }
        }
        
        if f_name_str == "first" && args.is_empty() {
            let res = if let Some(arr) = j.as_array() {
                if let Some(first) = arr.first() {
                    json_serde_to_value(first)
                } else {
                    Value::from_bool(false)
                }
            } else {
                Value::from_bool(false)
            };
            unsafe { locals[dst as usize].dec_ref(); }
            locals[dst as usize] = res;
            return OpResult::Continue;
        }

        let pp = if f_name_str.starts_with('/') {
            normalize_json_path(&f_name_str)
        } else {
            normalize_json_path(&format!("/{}", f_name_str))
        };
        let res = if let Some(v) = j.pointer(&pp) {
            json_serde_to_value(v)
        } else {
            Value::from_bool(false)
        };
        unsafe { locals[dst as usize].dec_ref(); }
        locals[dst as usize] = res;
        OpResult::Continue
    }

    #[inline(never)]
    fn handle_fiber_method<'a>(&mut self, dst: u8, fiber_rc: Arc<RwLock<FiberState>>, kind: MethodKind, args: &[Value], ip: usize, locals: &mut [Value], _vm_arc: &'a Arc<VM>, _glbs: &mut Option<RwLockWriteGuard<'a, Vec<Value>>>) -> OpResult {
        // Fiber method handling logic.
        match kind {
            MethodKind::Next => {
                let mut f_write = fiber_rc.write();
                if let Some(val) = f_write.yielded_value.take() {
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = val;
                    return OpResult::Continue;
                }
                if f_write.is_done {
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(false);
                    return OpResult::Continue;
                }
                drop(f_write);
                let arg = args.first().cloned();
                if let Some(v) = arg { if v.is_ptr() { unsafe { v.inc_ref(); } } }
                OpResult::Call(fiber_rc, arg, dst)
            }
            MethodKind::Run => {
                let f_read = fiber_rc.read();
                if f_read.is_done || f_read.yielded_value.is_some() {
                    unsafe { locals[dst as usize].dec_ref(); }
                    locals[dst as usize] = Value::from_bool(true);
                    return OpResult::Continue;
                }
                drop(f_read);
                let arg = args.first().cloned();
                if let Some(v) = arg { if v.is_ptr() { unsafe { v.inc_ref(); } } }
                OpResult::Call(fiber_rc, arg, dst)
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
                OpResult::Continue
            }
            MethodKind::Close => {
                fiber_rc.write().is_done = true;
                let res = Value::from_bool(true);
                unsafe { locals[dst as usize].dec_ref(); }
                locals[dst as usize] = res;
                OpResult::Continue
            }
            _ => { eprintln!("Method {:?} not supported for Fiber{}", kind, self.current_span_info(ip)); OpResult::Halt }
        }
    }

    fn check_start_recording(&mut self, target_ip: usize, threshold: usize) {
        if target_ip < self.hot_counts.len() {
            let hc = unsafe { self.hot_counts.get_unchecked_mut(target_ip) };
            *hc += 1;
            if *hc >= threshold && !self.is_recording && self.trace_cache[target_ip].is_none() {
                self.recording_trace = Some(Trace {
                    ops: vec![],
                    start_ip: target_ip,
                    native_ptr: AtomicPtr::new(std::ptr::null_mut()),
                    min_locals: 0,
                });
                self.is_recording = true;
            }
        }
    }

    fn process_finished_trace(&mut self, current_ip: usize) {
        if let Some(mut trace) = self.recording_trace.take() {
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

                let needed = trace.ops.iter().filter_map(|op| {
                    match op {
                        TraceOp::AddInt { dst, src1, src2 }
                        | TraceOp::SubInt { dst, src1, src2 }
                        | TraceOp::MulInt { dst, src1, src2 }
                        | TraceOp::PowInt { dst, src1, src2 }
                        | TraceOp::PowFloat { dst, src1, src2 }
                        | TraceOp::CmpInt { dst, src1, src2, .. }
                        | TraceOp::CmpFloat { dst, src1, src2, .. }
                        | TraceOp::And { dst, src1, src2 }
                        | TraceOp::Or { dst, src1, src2 }
                        | TraceOp::IntConcat { dst, src1, src2 }
                        | TraceOp::Has { dst, src1, src2 }
                        | TraceOp::AddFloat { dst, src1, src2 }
                        | TraceOp::SubFloat { dst, src1, src2 }
                        | TraceOp::MulFloat { dst, src1, src2 } => {
                            Some([*dst, *src1, *src2].iter().map(|r| *r as usize).max().unwrap())
                        }
                        TraceOp::DivInt { dst, src1, src2, .. }
                        | TraceOp::ModInt { dst, src1, src2, .. }
                        | TraceOp::DivFloat { dst, src1, src2, .. }
                        | TraceOp::ModFloat { dst, src1, src2, .. } => {
                            Some([*dst, *src1, *src2].iter().map(|r| *r as usize).max().unwrap())
                        }
                        TraceOp::LoadConst { dst, .. }
                        | TraceOp::ArraySize { dst, .. }
                        | TraceOp::SetSize { dst, .. } => Some(*dst as usize),
                        TraceOp::IncLocal { reg } => Some(*reg as usize),
                        TraceOp::GuardInt { reg, .. }
                        | TraceOp::GuardFloat { reg, .. }
                        | TraceOp::GuardTrue { reg, .. }
                        | TraceOp::GuardFalse { reg, .. } => Some(*reg as usize),
                        TraceOp::Move { dst, src }
                        | TraceOp::Not { dst, src }
                        | TraceOp::CastIntToFloat { dst, src }
                        | TraceOp::RandomChoice { dst, src } => Some((*dst).max(*src) as usize),
                        TraceOp::GetVar { dst, .. } => Some(*dst as usize),
                        TraceOp::SetVar { src, .. } => Some(*src as usize),
                        TraceOp::LoopNextInt { reg, limit_reg, .. } => {
                            Some((*reg).max(*limit_reg) as usize)
                        }
                        TraceOp::IncLocalLoopNext { inc_reg, reg, limit_reg, .. } => {
                            Some(*inc_reg.max(reg).max(limit_reg) as usize)
                        }
                        TraceOp::IncVarLoopNext { reg, limit_reg, .. } => {
                            Some((*reg).max(*limit_reg) as usize)
                        }
                        TraceOp::ArrayGet { dst, arr_reg, idx_reg, .. } => {
                            Some(*dst.max(arr_reg).max(idx_reg) as usize)
                        }
                        TraceOp::ArrayPush { arr_reg, val_reg } => {
                            Some((*arr_reg).max(*val_reg) as usize)
                        }
                        TraceOp::SetContains { dst, set_reg, val_reg } => {
                            Some(*dst.max(set_reg).max(val_reg) as usize)
                        }
                        TraceOp::RandomInt { dst, min, max, step, .. }
                        | TraceOp::RandomFloat { dst, min, max, step, .. } => {
                            Some(*dst.max(min).max(max).max(step) as usize)
                        }
                        _ => None,
                    }
                }).max().unwrap_or(0);
                trace.min_locals = (needed + 1).max(10);

                let arc_trace = Arc::new(trace);
                self.vm.traces.write().insert(current_ip, arc_trace.clone());
                self.trace_cache[current_ip] = Some(arc_trace);
            } else {
                self.recording_trace = Some(trace);
                self.is_recording = true;
            }
        }
    }

    fn record_op(&mut self, op: OpCode, current_ip: usize, locals: &[Value]) {
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
                OpCode::Pow { dst, src1, src2 } => {
                    let a = locals[src1 as usize];
                    let b = locals[src2 as usize];
                    if a.is_int() && b.is_int() {
                        trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                        trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                        trace.ops.push(TraceOp::PowInt { dst, src1, src2 });
                    } else if a.is_float() && b.is_float() {
                        trace.ops.push(TraceOp::GuardFloat { reg: src1, ip: current_ip });
                        trace.ops.push(TraceOp::GuardFloat { reg: src2, ip: current_ip });
                        trace.ops.push(TraceOp::PowFloat { dst, src1, src2 });
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
                OpCode::RandomInt { dst, min, max, step, has_step } => {
                    trace.ops.push(TraceOp::GuardInt { reg: min, ip: current_ip });
                    trace.ops.push(TraceOp::GuardInt { reg: max, ip: current_ip });
                    if locals[has_step as usize].as_bool() {
                        trace.ops.push(TraceOp::GuardInt { reg: step, ip: current_ip });
                    }
                    trace.ops.push(TraceOp::RandomInt { dst, min, max, step, has_step });
                }
                OpCode::RandomFloat { dst, min, max, step, has_step } => {
                    trace.ops.push(TraceOp::GuardFloat { reg: min, ip: current_ip });
                    trace.ops.push(TraceOp::GuardFloat { reg: max, ip: current_ip });
                    let mut step_is_float = false;
                    if locals[has_step as usize].as_bool() {
                        let s = locals[step as usize];
                        if s.is_float() {
                            trace.ops.push(TraceOp::GuardFloat { reg: step, ip: current_ip });
                            step_is_float = true;
                        } else {
                            trace.ops.push(TraceOp::GuardInt { reg: step, ip: current_ip });
                        }
                    }
                    trace.ops.push(TraceOp::RandomFloat { dst, min, max, step, has_step, step_is_float });
                }
                OpCode::IntConcat { dst, src1, src2 } => {
                    trace.ops.push(TraceOp::GuardInt { reg: src1, ip: current_ip });
                    trace.ops.push(TraceOp::GuardInt { reg: src2, ip: current_ip });
                    trace.ops.push(TraceOp::IntConcat { dst, src1, src2 });
                }
                OpCode::Has { dst, src1, src2 } => {
                    trace.ops.push(TraceOp::Has { dst, src1, src2 });
                }
                OpCode::RandomChoice { dst, src } => {
                    trace.ops.push(TraceOp::RandomChoice { dst, src });
                }
                OpCode::MethodCall { dst, kind, base, .. } => {
                    let receiver = locals[base as usize];
                    match kind {
                        MethodKind::Size | MethodKind::Len | MethodKind::Length | MethodKind::Count => {
                            if receiver.is_array() {
                                trace.ops.push(TraceOp::ArraySize { dst, src: base });
                            } else if receiver.is_set() {
                                trace.ops.push(TraceOp::SetSize { dst, src: base });
                            } else {
                                self.recording_trace = None;
                                self.is_recording = false;
                            }
                        }
                        MethodKind::Get => {
                            if receiver.is_array() {
                                trace.ops.push(TraceOp::ArrayGet { dst, arr_reg: base, idx_reg: base + 1, fail_ip: current_ip });
                            } else {
                                self.recording_trace = None;
                                self.is_recording = false;
                            }
                        }
                        MethodKind::Push | MethodKind::Add => {
                            if receiver.is_array() {
                                trace.ops.push(TraceOp::ArrayPush { arr_reg: base, val_reg: base + 1 });
                            } else {
                                self.recording_trace = None;
                                self.is_recording = false;
                            }
                        }
                        MethodKind::Update | MethodKind::Set => {
                            if receiver.is_array() {
                                trace.ops.push(TraceOp::ArrayUpdate { arr_reg: base, idx_reg: base + 1, val_reg: base + 2, fail_ip: current_ip });
                            } else {
                                self.recording_trace = None;
                                self.is_recording = false;
                            }
                        }
                        MethodKind::Contains | MethodKind::Has => {
                            if receiver.is_set() {
                                trace.ops.push(TraceOp::SetContains { dst, set_reg: base, val_reg: base + 1 });
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
                _ => {
                    self.recording_trace = None;
                    self.is_recording = false;
                }
            }
        }
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
        Err(ureq::Error::Status(code, resp)) => {
            let mut h_map = serde_json::Map::new();
            for name in resp.headers_names() {
                if let Some(val) = resp.header(&name) {
                    h_map.insert(name, serde_json::Value::String(val.to_string()));
                }
            }
            let text = resp.into_string().unwrap_or_default();
            let body_val = serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));
            let mut res = serde_json::Map::new();
            res.insert("status".to_string(),  serde_json::Value::Number(code.into()));
            res.insert("ok".to_string(),      serde_json::Value::Bool(false));
            res.insert("error".to_string(),   serde_json::Value::String(format!("Status code {}", code)));
            res.insert("body".to_string(),    body_val);
            res.insert("headers".to_string(), serde_json::Value::Object(h_map));
            serde_json::Value::Object(res)
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
        serde_json::Value::String(s) => Value::from_string(Arc::new(s.clone().into_bytes())),
        serde_json::Value::Array(arr) => {
            let mut vals = Vec::with_capacity(arr.len());
            for x in arr {
                vals.push(json_serde_to_value(x));
            }
            Value::from_array(Arc::new(RwLock::new(vals)))
        }
        serde_json::Value::Object(obj) => {
            let mut map = Vec::with_capacity(obj.len());
            for (k, v) in obj {
                map.push((Value::from_string(Arc::new(k.clone().into_bytes())), json_serde_to_value(v)));
            }
            Value::from_map(Arc::new(RwLock::new(map)))
        }
    }
}

fn get_path_value_xcx(root: Value, path: &str) -> Value {
    let pp = normalize_json_path(path);
    if pp.is_empty() || pp == "/" {
        unsafe { root.inc_ref(); }
        return root;
    }
    let parts: Vec<&str> = pp.split('/').filter(|s| !s.is_empty()).collect();
    let mut current = root;
    unsafe { current.inc_ref(); }

    for part in parts {
        let next = if current.is_array() {
            let idx = part.parse::<usize>().unwrap_or(u32::MAX as usize);
            let arr_rc = current.as_array();
            let arr = arr_rc.read();
            if idx < arr.len() {
                let v = arr[idx];
                unsafe { v.inc_ref(); }
                v
            } else { Value::from_bool(false) }
        } else if current.is_map() {
            let map_rc = current.as_map();
            let map = map_rc.read();
            if let Some((_, v)) = map.iter().find(|(k, _)| k.matches_str(part)) {
                unsafe { v.inc_ref(); }
                *v
            } else { Value::from_bool(false) }
        } else if (current.0 & 0x000F_0000_0000_0000) == TAG_JSON {
            let json_rc = current.as_json();
            let json = json_rc.read();
            let v = if let Some(idx) = part.parse::<usize>().ok() {
                json.get(idx)
            } else {
                json.get(part)
            };
            match v {
                Some(v) => json_serde_to_value(v),
                None => Value::from_bool(false),
            }
        } else {
            Value::from_bool(false)
        };
        unsafe { current.dec_ref(); }
        current = next;
        if !current.is_ptr() && !current.is_int() && !current.is_float() && !current.is_bool() { break; }
    }
    current
}

fn set_path_value_xcx(root: Value, path: &str, value: Value) {
    let pp = normalize_json_path(path);
    let parts: Vec<&str> = pp.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() { return; }
    
    let mut current = root;
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        if current.is_array() {
            let idx = part.parse::<usize>().unwrap_or(u32::MAX as usize);
            let arr_rc = current.as_array();
            if is_last {
                let mut arr = arr_rc.write();
                if idx < arr.len() {
                    let old = arr[idx];
                    unsafe { value.inc_ref(); }
                    arr[idx] = value;
                    unsafe { old.dec_ref(); }
                } else if idx == arr.len() {
                    unsafe { value.inc_ref(); }
                    arr.push(value);
                }
                return;
            }
            let arr = arr_rc.read();
            if idx < arr.len() && (arr[idx].is_array() || arr[idx].is_map()) {
                current = arr[idx];
            } else { return; }
        } else if current.is_map() {
            let map_rc = current.as_map();
            if is_last {
                let mut map = map_rc.write();
                if let Some(e) = map.iter_mut().find(|(k, _)| k.to_string() == *part) {
                    let old_v = e.1;
                    unsafe { value.inc_ref(); }
                    e.1 = value;
                    unsafe { old_v.dec_ref(); }
                } else {
                    let key = Value::from_string(Arc::new(part.to_string().into_bytes()));
                    unsafe { key.inc_ref(); value.inc_ref(); }
                    map.push((key, value));
                }
                return;
            }
            let map = map_rc.read();
            if let Some((_, v)) = map.iter().find(|(k, _)| k.to_string() == *part) {
                if v.is_map() || v.is_array() {
                    current = *v;
                } else { return; }
            } else { return; }
        } else { return; }
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
        TAG_STR => {
            let b = v.as_string();
            match String::from_utf8((*b).clone()) {
                Ok(s) => serde_json::Value::String(s),
                Err(_) => serde_json::Value::String(hex::encode(&*b)) 
            }
        }
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
        TAG_TBL => {
            let t_rc = v.as_table();
            let t = t_rc.read();
            t.to_json()
        }
        TAG_ROW => {
            let r = v.as_row();
            let t = r.table.read();
            let row_idx = r.row_idx as usize;
            if row_idx < t.rows.len() {
                let mut obj = serde_json::Map::new();
                for (i, col) in t.columns.iter().enumerate() {
                    if i < t.rows[row_idx].len() {
                        obj.insert(col.name.clone(), value_to_json(&t.rows[row_idx][i]));
                    }
                }
                serde_json::Value::Object(obj)
            } else {
                serde_json::Value::Null
            }
        }
        TAG_JSON => v.as_json().read().clone(),
        TAG_DATE => {
            let ts = v.as_date();
            let dt = chrono::DateTime::from_timestamp_millis(ts).unwrap().naive_utc();
            serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())
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
    }
    #[test]
    fn test_opcode_size() {
        println!("OpCode size: {}", std::mem::size_of::<OpCode>());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_random_int(min: i64, max: i64, step: i64, has_step: bool) -> i64 {
    let mut rng = rand::rng();
    let diff = max - min;
    let abs_diff = diff.abs();
    let abs_step = if has_step { step.abs().max(1) } else { 1 };
    let steps = abs_diff / abs_step;
    let k = rng.random_range(0..=steps);
    let sign = if diff >= 0 { 1 } else { -1 };
    min + k * sign * abs_step
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_random_float(min: f64, max: f64, step: f64, has_step: bool) -> f64 {
    let mut rng = rand::rng();
    let diff = max - min;
    let abs_diff = diff.abs();
    let abs_step = if has_step { step.abs() } else { 0.5 };
    
    if abs_step > 0.0 {
        let steps = (abs_diff / abs_step).floor() as i64;
        let k = rng.random_range(0..=steps);
        let sign = if diff >= 0.0 { 1.0 } else { -1.0 };
        min + (k as f64) * sign * abs_step
    } else {
        use rand::Rng;
        let t: f64 = rng.random();
        min + t * diff
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_pow_int(a: i64, b: i64) -> i64 {
    a.pow(b as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_pow_float(a: f64, b: f64) -> f64 {
    a.powf(b)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_int_concat(a: i64, b: i64) -> i64 {
    let b_digits = if b == 0 { 1 } else { (b.abs() as f64).log10().floor() as u32 + 1 };
    a * 10i64.pow(b_digits) + b
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_has(container_bits: u64, item_bits: u64) -> bool {
    let container = Value(container_bits);
    let item = Value(item_bits);
    if container.is_string() && item.is_string() {
        container.to_string().contains(&item.to_string())
    } else if container.is_array() {
        let arc = container.as_array();
        arc.read().iter().any(|v| v == &item)
    } else if container.is_ptr() && (container.0 & 0x000F_0000_0000_0000) == TAG_SET {
        let arc = container.as_set();
        arc.read().elements.contains(&item)
    } else if container.is_map() {
        let arc = container.as_map();
        arc.read().iter().any(|(k, _)| k == &item)
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_random_choice(col_bits: u64) -> u64 {
    let col = Value(col_bits);
    if col.is_ptr() {
        let mut rng = rand::rng();
        match col.0 & 0x000F_0000_0000_0000 {
            TAG_ARR => {
                let arc = col.as_array();
                let arr = arc.read();
                if arr.is_empty() { Value::from_bool(false).0 }
                else { let v = arr[rng.random_range(0..arr.len())]; unsafe { v.inc_ref(); } v.0 }
            }
            TAG_SET => {
                let arc = col.as_set();
                let mut s_write = arc.write();
                if s_write.cache.is_none() {
                    s_write.cache = Some(s_write.elements.iter().cloned().collect());
                }
                let cache = s_write.cache.as_ref().unwrap();
                if cache.is_empty() { Value::from_bool(false).0 }
                else { let v = cache[rng.random_range(0..cache.len())]; unsafe { v.inc_ref(); } v.0 }
            }
            TAG_MAP => {
                let arc = col.as_map();
                let map = arc.read();
                if map.is_empty() { Value::from_bool(false).0 }
                else {
                    let (k, _) = map[rng.random_range(0..map.len())];
                    unsafe { k.inc_ref(); }
                    k.0
                }
            }
            _ => Value::from_bool(false).0
        }
    } else {
        Value::from_bool(false).0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_array_size(arr_bits: u64) -> i64 {
    let arr = Value(arr_bits);
    let arc = arr.as_array();
    arc.read().len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_array_get(arr_bits: u64, idx: i64) -> u64 {
    let arr = Value(arr_bits);
    let arc = arr.as_array();
    let arr_read = arc.read();
    if idx < 0 || idx >= arr_read.len() as i64 {
        return Value(0).0;
    }
    let val = arr_read[idx as usize];
    unsafe { val.inc_ref(); }
    val.0
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_array_push(arr_bits: u64, val_bits: u64) {
    let arr = Value(arr_bits);
    let val = Value(val_bits);
    unsafe { val.inc_ref(); }
    let arc = arr.as_array();
    arc.write().push(val);
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_inc_ref(v_bits: u64) {
    let v = Value(v_bits);
    unsafe { v.inc_ref(); }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_dec_ref(v_bits: u64) {
    let v = Value(v_bits);
    unsafe { v.dec_ref(); }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_set_size(set_bits: u64) -> i64 {
    let set = Value(set_bits);
    let arc = set.as_set();
    arc.read().elements.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_set_contains(set_bits: u64, val_bits: u64) -> bool {
    let set = Value(set_bits);
    let val = Value(val_bits);
    let arc = set.as_set();
    arc.read().elements.contains(&val)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_array_update(arr_bits: u64, idx: i64, val_bits: u64) -> i32 {
    let arr = Value(arr_bits);
    let val = Value(val_bits);
    let arc = arr.as_array();
    let mut arr_write = arc.write();
    if idx < 0 || idx >= arr_write.len() as i64 {
        return 0;
    }
    unsafe { val.inc_ref(); }
    let old = arr_write[idx as usize];
    arr_write[idx as usize] = val;
    unsafe { old.dec_ref(); }
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_call_recursive(
    func_id_idx: usize,
    params_ptr: *const Value,
    params_count: u8,
    _vm: *const VM,
    executor: *mut Executor,
    globals_ptr: *mut Value,
) -> u64 {
    let executor = unsafe { &mut *executor };
    let params = unsafe { std::slice::from_raw_parts(params_ptr, params_count as usize) };
    
    let chunk = executor.ctx.functions[func_id_idx].clone();
    
    // Fast path: if already JIT-compiled, call directly using pooled locals.
    // Bypasses run_frame_with_guard to avoid redundant locking and tracing overhead.
    let jit_ptr = chunk.jit_ptr.load(Ordering::Relaxed);
    if !jit_ptr.is_null() && !executor.is_recording {
        let jit_fn: crate::backend::jit::MethodJitFunction = unsafe { std::mem::transmute(jit_ptr) };
        let mut locals = executor.locals_pool.pop().unwrap_or_else(|| Vec::with_capacity(chunk.max_locals.max(params.len())));
        locals.clear();
        for &v in params {
            unsafe { v.inc_ref(); }
            locals.push(v);
        }
        locals.resize(chunk.max_locals.max(params.len()), Value::from_bool(false));
        
        let res_bits = unsafe {
            jit_fn(locals.as_mut_ptr(), globals_ptr, executor.ctx.constants.as_ptr(), _vm as *mut VM, executor)
        };
        
        for v in locals.iter() {
            unsafe { v.dec_ref(); }
        }
        executor.locals_pool.push(locals);
        return res_bits;
    }
    
    let vm_arc = executor.vm.clone();
    let res = executor.run_frame_with_guard(chunk, params, &vm_arc, &mut None, func_id_idx);
    res.map(|v| v.0).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_method_dispatch(
    dst: u8,
    kind_raw: u8,
    receiver_bits: u64,
    args_ptr: *const Value,
    arg_count: u8,
    locals_ptr: *mut Value,
    executor_ptr: *mut Executor,
) {
    let receiver = Value(receiver_bits);
    let kind = unsafe { std::mem::transmute::<u8, MethodKind>(kind_raw) };
    let args = unsafe { std::slice::from_raw_parts(args_ptr, arg_count as usize) };
    let executor = unsafe { &mut *executor_ptr };
    
    // Method JIT locals pointer. We assume enough capacity (mapped to 0..255).
    let locals = unsafe { std::slice::from_raw_parts_mut(locals_ptr, 256) };
    
    let vm_arc = executor.vm.clone();
    let mut glbs = None; // Global lock is handled outside if needed, but methods usually don't need it for locals access.
    
    if receiver.is_db() {
        executor.handle_database_method(dst, receiver.as_db(), kind, args, None, 0, locals, &vm_arc, &mut glbs);
    } else {
        executor.handle_method_call(dst, receiver, kind, args, None, 0, locals, &vm_arc, &mut glbs);
    }
}

// Ensure JIT-only helpers are not stripped by the linker
#[inline(never)]
pub fn preserve_jit_helpers_dummy() {
    if std::env::var("XCX_PRESERVE_JIT").is_ok() {
        let v = Value::from_bool(false);
        xcx_jit_inc_ref(v.0);
        xcx_jit_dec_ref(v.0);
        xcx_jit_method_dispatch(0, 0, v.0, std::ptr::null(), 0, std::ptr::null_mut(), std::ptr::null_mut());
    }
}

fn validate_path_safety(path: &str) {
    if path.contains("..") || path.starts_with('/') || (path.len() > 1 && path.as_bytes()[1] == b':') {
        eprintln!("HALT.FATAL: Security violation - illegal path access: {}", path);
        std::process::exit(1);
    }
}

fn zip_folder(source: &str, target: &str) -> std::io::Result<()> {
    let file = std::fs::File::create(target)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let src_path = std::path::Path::new(source);
    if src_path.is_file() {
        zip.start_file(src_path.file_name().unwrap().to_str().unwrap(), options)?;
        let mut f = std::fs::File::open(src_path)?;
        std::io::copy(&mut f, &mut zip)?;
    } else {
        for entry in walkdir::WalkDir::new(source) {
            let entry = entry?;
            let path = entry.path();
            let name = path.strip_prefix(src_path).unwrap();

            // Skip the target zip file itself to avoid recursion/corruption
            // and skip the sensitive token file.
            if let Some(target_path) = std::path::Path::new(target).file_name() {
                if path.file_name() == Some(target_path) { continue; }
            }
            if path.file_name().map_or(false, |n| n == ".pax_token") {
                continue;
            }

            if path.is_file() {
                zip.start_file(name.to_str().unwrap(), options)?;
                let mut f = std::fs::File::open(path)?;
                std::io::copy(&mut f, &mut zip)?;
            } else if !name.as_os_str().is_empty() {
                zip.add_directory(name.to_str().unwrap(), options)?;
            }
        }
    }
    zip.finish()?;
    Ok(())
}

fn unzip_archive(zip_file: &str, dest_dir: &str) -> std::io::Result<()> {
    let file = std::fs::File::open(zip_file)?;
    let mut archive = zip::ZipArchive::new(file)?;
    std::fs::create_dir_all(dest_dir)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => std::path::Path::new(dest_dir).join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}
