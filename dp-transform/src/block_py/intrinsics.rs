use std::any::Any;
use std::fmt;

pub trait Intrinsic: Any + fmt::Debug + Sync {
    fn name(&self) -> &'static str;

    fn arity(&self) -> usize;

    fn accepts_arity(&self, arity: usize) -> bool {
        arity == self.arity()
    }

    fn as_any(&self) -> &dyn Any;
}

macro_rules! define_intrinsic {
    ($struct_name:ident, $static_name:ident, $name:literal, $arity:literal) => {
        #[derive(Debug)]
        pub struct $struct_name;

        impl Intrinsic for $struct_name {
            fn name(&self) -> &'static str {
                $name
            }

            fn arity(&self) -> usize {
                $arity
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        pub static $static_name: $struct_name = $struct_name;
    };
    ($struct_name:ident, $static_name:ident, $name:literal, [$($arity:literal),+ $(,)?]) => {
        #[derive(Debug)]
        pub struct $struct_name;

        impl Intrinsic for $struct_name {
            fn name(&self) -> &'static str {
                $name
            }

            fn arity(&self) -> usize {
                define_intrinsic!(@first $($arity),+)
            }

            fn accepts_arity(&self, arity: usize) -> bool {
                matches!(arity, $($arity)|+)
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        pub static $static_name: $struct_name = $struct_name;
    };
    (@first $value:literal $(, $rest:literal)*) => {
        $value
    };
}

define_intrinsic!(AddIntrinsic, ADD_INTRINSIC, "__dp_add", 2);
define_intrinsic!(GetAttrIntrinsic, GETATTR_INTRINSIC, "__dp_getattr", 2);
define_intrinsic!(SetAttrIntrinsic, SETATTR_INTRINSIC, "__dp_setattr", 3);
define_intrinsic!(GetItemIntrinsic, GETITEM_INTRINSIC, "__dp_getitem", 2);
define_intrinsic!(SetItemIntrinsic, SETITEM_INTRINSIC, "__dp_setitem", 3);
define_intrinsic!(DelItemIntrinsic, DELITEM_INTRINSIC, "__dp_delitem", 2);
define_intrinsic!(
    LoadGlobalIntrinsic,
    LOAD_GLOBAL_INTRINSIC,
    "__dp_load_global",
    2
);
define_intrinsic!(
    StoreGlobalIntrinsic,
    STORE_GLOBAL_INTRINSIC,
    "__dp_store_global",
    3
);
define_intrinsic!(LoadCellIntrinsic, LOAD_CELL_INTRINSIC, "__dp_load_cell", 1);
define_intrinsic!(MakeCellIntrinsic, MAKE_CELL_INTRINSIC, "__dp_make_cell", 1);
define_intrinsic!(CellRefIntrinsic, CELL_REF_INTRINSIC, "__dp_cell_ref", 1);
define_intrinsic!(
    StoreCellIntrinsic,
    STORE_CELL_INTRINSIC,
    "__dp_store_cell",
    2
);
define_intrinsic!(
    DelQuietlyIntrinsic,
    DEL_QUIETLY_INTRINSIC,
    "__dp_del_quietly",
    2
);
define_intrinsic!(
    DelDerefQuietlyIntrinsic,
    DEL_DEREF_QUIETLY_INTRINSIC,
    "__dp_del_deref_quietly",
    1
);
define_intrinsic!(DelDerefIntrinsic, DEL_DEREF_INTRINSIC, "__dp_del_deref", 1);
define_intrinsic!(SubIntrinsic, SUB_INTRINSIC, "__dp_sub", 2);
define_intrinsic!(MulIntrinsic, MUL_INTRINSIC, "__dp_mul", 2);
define_intrinsic!(MatMulIntrinsic, MATMUL_INTRINSIC, "__dp_matmul", 2);
define_intrinsic!(TrueDivIntrinsic, TRUEDIV_INTRINSIC, "__dp_truediv", 2);
define_intrinsic!(FloorDivIntrinsic, FLOORDIV_INTRINSIC, "__dp_floordiv", 2);
define_intrinsic!(ModIntrinsic, MOD_INTRINSIC, "__dp_mod", 2);
define_intrinsic!(PowIntrinsic, POW_INTRINSIC, "__dp_pow", [2, 3]);
define_intrinsic!(LShiftIntrinsic, LSHIFT_INTRINSIC, "__dp_lshift", 2);
define_intrinsic!(RShiftIntrinsic, RSHIFT_INTRINSIC, "__dp_rshift", 2);
define_intrinsic!(OrIntrinsic, OR_INTRINSIC, "__dp_or_", 2);
define_intrinsic!(XorIntrinsic, XOR_INTRINSIC, "__dp_xor", 2);
define_intrinsic!(AndIntrinsic, AND_INTRINSIC, "__dp_and_", 2);
define_intrinsic!(InPlaceAddIntrinsic, IADD_INTRINSIC, "__dp_iadd", 2);
define_intrinsic!(InPlaceSubIntrinsic, ISUB_INTRINSIC, "__dp_isub", 2);
define_intrinsic!(InPlaceMulIntrinsic, IMUL_INTRINSIC, "__dp_imul", 2);
define_intrinsic!(InPlaceMatMulIntrinsic, IMATMUL_INTRINSIC, "__dp_imatmul", 2);
define_intrinsic!(
    InPlaceTrueDivIntrinsic,
    ITRUEDIV_INTRINSIC,
    "__dp_itruediv",
    2
);
define_intrinsic!(
    InPlaceFloorDivIntrinsic,
    IFLOORDIV_INTRINSIC,
    "__dp_ifloordiv",
    2
);
define_intrinsic!(InPlaceModIntrinsic, IMOD_INTRINSIC, "__dp_imod", 2);
define_intrinsic!(InPlacePowIntrinsic, IPOW_INTRINSIC, "__dp_ipow", [2, 3]);
define_intrinsic!(InPlaceLShiftIntrinsic, ILSHIFT_INTRINSIC, "__dp_ilshift", 2);
define_intrinsic!(InPlaceRShiftIntrinsic, IRSHIFT_INTRINSIC, "__dp_irshift", 2);
define_intrinsic!(InPlaceOrIntrinsic, IOR_INTRINSIC, "__dp_ior", 2);
define_intrinsic!(InPlaceXorIntrinsic, IXOR_INTRINSIC, "__dp_ixor", 2);
define_intrinsic!(InPlaceAndIntrinsic, IAND_INTRINSIC, "__dp_iand", 2);
define_intrinsic!(PosIntrinsic, POS_INTRINSIC, "__dp_pos", 1);
define_intrinsic!(NegIntrinsic, NEG_INTRINSIC, "__dp_neg", 1);
define_intrinsic!(InvertIntrinsic, INVERT_INTRINSIC, "__dp_invert", 1);
define_intrinsic!(NotIntrinsic, NOT_INTRINSIC, "__dp_not_", 1);
define_intrinsic!(TruthIntrinsic, TRUTH_INTRINSIC, "__dp_truth", 1);
define_intrinsic!(EqIntrinsic, EQ_INTRINSIC, "__dp_eq", 2);
define_intrinsic!(NeIntrinsic, NE_INTRINSIC, "__dp_ne", 2);
define_intrinsic!(LtIntrinsic, LT_INTRINSIC, "__dp_lt", 2);
define_intrinsic!(LeIntrinsic, LE_INTRINSIC, "__dp_le", 2);
define_intrinsic!(GtIntrinsic, GT_INTRINSIC, "__dp_gt", 2);
define_intrinsic!(GeIntrinsic, GE_INTRINSIC, "__dp_ge", 2);
define_intrinsic!(ContainsIntrinsic, CONTAINS_INTRINSIC, "__dp_contains", 2);
define_intrinsic!(IsIntrinsic, IS_INTRINSIC, "__dp_is_", 2);
define_intrinsic!(IsNotIntrinsic, IS_NOT_INTRINSIC, "__dp_is_not", 2);

pub fn intrinsic_by_name_and_arity(name: &str, arity: usize) -> Option<&'static dyn Intrinsic> {
    let intrinsic: &'static dyn Intrinsic = match name {
        "__dp_add" => &ADD_INTRINSIC,
        "__dp_getattr" => &GETATTR_INTRINSIC,
        "__dp_setattr" => &SETATTR_INTRINSIC,
        "__dp_getitem" => &GETITEM_INTRINSIC,
        "__dp_setitem" => &SETITEM_INTRINSIC,
        "__dp_delitem" => &DELITEM_INTRINSIC,
        "__dp_load_global" => &LOAD_GLOBAL_INTRINSIC,
        "__dp_store_global" => &STORE_GLOBAL_INTRINSIC,
        "__dp_load_cell" => &LOAD_CELL_INTRINSIC,
        "__dp_make_cell" => &MAKE_CELL_INTRINSIC,
        "__dp_cell_ref" => &CELL_REF_INTRINSIC,
        "__dp_store_cell" => &STORE_CELL_INTRINSIC,
        "__dp_del_quietly" => &DEL_QUIETLY_INTRINSIC,
        "__dp_del_deref_quietly" => &DEL_DEREF_QUIETLY_INTRINSIC,
        "__dp_del_deref" => &DEL_DEREF_INTRINSIC,
        "__dp_sub" => &SUB_INTRINSIC,
        "__dp_mul" => &MUL_INTRINSIC,
        "__dp_matmul" => &MATMUL_INTRINSIC,
        "__dp_truediv" => &TRUEDIV_INTRINSIC,
        "__dp_floordiv" => &FLOORDIV_INTRINSIC,
        "__dp_mod" => &MOD_INTRINSIC,
        "__dp_pow" => &POW_INTRINSIC,
        "__dp_lshift" => &LSHIFT_INTRINSIC,
        "__dp_rshift" => &RSHIFT_INTRINSIC,
        "__dp_or_" => &OR_INTRINSIC,
        "__dp_xor" => &XOR_INTRINSIC,
        "__dp_and_" => &AND_INTRINSIC,
        "__dp_iadd" => &IADD_INTRINSIC,
        "__dp_isub" => &ISUB_INTRINSIC,
        "__dp_imul" => &IMUL_INTRINSIC,
        "__dp_imatmul" => &IMATMUL_INTRINSIC,
        "__dp_itruediv" => &ITRUEDIV_INTRINSIC,
        "__dp_ifloordiv" => &IFLOORDIV_INTRINSIC,
        "__dp_imod" => &IMOD_INTRINSIC,
        "__dp_ipow" => &IPOW_INTRINSIC,
        "__dp_ilshift" => &ILSHIFT_INTRINSIC,
        "__dp_irshift" => &IRSHIFT_INTRINSIC,
        "__dp_ior" => &IOR_INTRINSIC,
        "__dp_ixor" => &IXOR_INTRINSIC,
        "__dp_iand" => &IAND_INTRINSIC,
        "__dp_pos" => &POS_INTRINSIC,
        "__dp_neg" => &NEG_INTRINSIC,
        "__dp_invert" => &INVERT_INTRINSIC,
        "__dp_not_" => &NOT_INTRINSIC,
        "__dp_truth" => &TRUTH_INTRINSIC,
        "__dp_eq" => &EQ_INTRINSIC,
        "__dp_ne" => &NE_INTRINSIC,
        "__dp_lt" => &LT_INTRINSIC,
        "__dp_le" => &LE_INTRINSIC,
        "__dp_gt" => &GT_INTRINSIC,
        "__dp_ge" => &GE_INTRINSIC,
        "__dp_contains" => &CONTAINS_INTRINSIC,
        "__dp_is_" => &IS_INTRINSIC,
        "__dp_is_not" => &IS_NOT_INTRINSIC,
        _ => return None,
    };
    intrinsic.accepts_arity(arity).then_some(intrinsic)
}
