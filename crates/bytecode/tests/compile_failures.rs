mod bytecode {
    use koto_bytecode::{Compiler, CompilerSettings};

    fn check_compilation_fails(source: &str) {
        if Compiler::compile(source, None, CompilerSettings::default()).is_ok() {
            panic!("\nUnexpected success while compiling: {source}");
        }
    }

    mod should_fail {
        use super::*;

        #[test]
        fn wildcard_access() {
            let source = "
f _x
";
            check_compilation_fails(source);
        }

        #[test]
        fn wildcard_as_rhs() {
            let source = "
x = 1 + _
";
            check_compilation_fails(source);
        }

        #[test]
        fn break_outside_of_loop() {
            let source = "
break
";
            check_compilation_fails(source);
        }

        #[test]
        fn continue_outside_of_loop() {
            let source = "
continue
";
            check_compilation_fails(source);
        }

        #[test]
        fn ellipsis_outside_of_call() {
            let source = "
a...
";
            check_compilation_fails(source);
        }

        mod try_catch {
            use super::*;

            #[test]
            fn missing_type_hint_on_first_catch_block() {
                let source = "
try
  f()
catch x
  x
catch y
  y
                    ";
                check_compilation_fails(source);
            }

            #[test]
            fn missing_type_hint_on_first_catch_block_with_wildcard_arg() {
                let source = "
try
  f()
catch _x
  0
catch y
  y
                    ";
                check_compilation_fails(source);
            }

            #[test]
            fn type_hint_on_last_catch_block() {
                let source = "
try
  f()
catch x: String
  x
catch x: Bool
  x
                    ";
                check_compilation_fails(source);
            }

            #[test]
            fn type_hint_on_last_catch_block_with_wildcard_arg() {
                let source = "
try
  f()
catch x: String
  x
catch _x: Bool
  0
                    ";
                check_compilation_fails(source);
            }
        }

        mod match_failures {
            use super::*;

            #[test]
            fn match_insufficient_patterns() {
                let source = "
match 0, 1
  x then x
";
                check_compilation_fails(source);
            }

            #[test]
            fn match_too_many_patterns() {
                let source = "
match 0
  x, y then x + y
";
                check_compilation_fails(source);
            }

            #[test]
            fn match_ellipsis_out_of_position() {
                let source = "
match [1, 2, 3]
  (x, ..., y) then 0
";
                check_compilation_fails(source);
            }
        }

        mod functions {
            use super::*;

            #[test]
            fn error_in_unused_function() {
                let source = "
|| break
true
";
                check_compilation_fails(source);
            }

            #[test]
            fn more_than_256_arguments() {
                let source = "
|
  x000, x001, x002, x003, x004, x005, x006, x007, x008, x009, x010, x011, x012, x013, x014, x015,
  x016, x017, x018, x019, x020, x021, x022, x023, x024, x025, x026, x027, x028, x029, x030, x031,
  x032, x033, x034, x035, x036, x037, x038, x039, x040, x041, x042, x043, x044, x045, x046, x047,
  x048, x049, x050, x051, x052, x053, x054, x055, x056, x057, x058, x059, x060, x061, x062, x063,
  x064, x065, x066, x067, x068, x069, x070, x071, x072, x073, x074, x075, x076, x077, x078, x079,
  x080, x081, x082, x083, x084, x085, x086, x087, x088, x089, x090, x091, x092, x093, x094, x095,
  x096, x097, x098, x099, x100, x101, x102, x103, x104, x105, x106, x107, x108, x109, x110, x111,
  x112, x113, x114, x115, x116, x117, x118, x119, x120, x121, x122, x123, x124, x125, x126, x127,
  x128, x129, x130, x131, x132, x133, x134, x135, x136, x137, x138, x139, x140, x141, x142, x143,
  x144, x145, x146, x147, x148, x149, x150, x151, x152, x153, x154, x155, x156, x157, x158, x159,
  x160, x161, x162, x163, x164, x165, x166, x167, x168, x169, x170, x171, x172, x173, x174, x175,
  x176, x177, x178, x179, x180, x181, x182, x183, x184, x185, x186, x187, x188, x189, x190, x191,
  x192, x193, x194, x195, x196, x197, x198, x199, x200, x201, x202, x203, x204, x205, x206, x207,
  x208, x209, x210, x211, x212, x213, x214, x215, x216, x217, x218, x219, x220, x221, x222, x223,
  x224, x225, x226, x227, x228, x229, x230, x231, x232, x233, x234, x235, x236, x237, x238, x239,
  x240, x241, x242, x243, x244, x245, x246, x247, x248, x249, x250, x251, x252, x253, x254, x255,
  x256, # too many arguments
| x
";
                check_compilation_fails(source);
            }
        }
    }
}
