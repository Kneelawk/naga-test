// use naga::back::wgsl;
use core::{mem, slice};
use naga::back::spv;
use naga::valid::{ValidationFlags, Validator};
use naga::{
    BinaryOperator, Binding, Constant, ConstantInner, EntryPoint, Expression, Function,
    FunctionArgument, FunctionResult, Module, Range, ScalarKind, ScalarValue, ShaderStage,
    Statement, Type, TypeInner, VectorSize,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

fn main() {
    println!("Creating module...");
    let mut module = Module::default();
    let vec_type = Type {
        name: None,
        inner: TypeInner::Vector {
            size: VectorSize::Bi,
            kind: ScalarKind::Float,
            width: 4,
        },
    };
    let vec_type = module.types.append(vec_type);

    let mut function = Function::default();
    function.arguments.push(FunctionArgument {
        name: Some("a".to_string()),
        ty: vec_type.clone(),
        binding: Some(Binding::Location {
            location: 0,
            interpolation: None,
            sampling: None,
        }),
    });
    function.name = Some("main".to_string());
    let a_arg = function.expressions.append(Expression::FunctionArgument(0));
    let const_1 = module.constants.append(Constant {
        name: None,
        specialization: None,
        inner: ConstantInner::Scalar {
            width: 4,
            value: ScalarValue::Float(1.0),
        },
    });
    let const_vec_1 = module.constants.append(Constant {
        name: None,
        specialization: None,
        inner: ConstantInner::Composite {
            ty: vec_type,
            components: vec![const_1, const_1],
        },
    });
    let const_vec_1_expr = function
        .expressions
        .append(Expression::Constant(const_vec_1));
    let len = function.expressions.len();
    let add = function.expressions.append(Expression::Binary {
        op: BinaryOperator::Add,
        left: a_arg,
        right: const_vec_1_expr,
    });
    let range = function.expressions.range_from(len);
    function.body.push(Statement::Emit(range));
    function.body.push(Statement::Return { value: Some(add) });
    function.result = Some(FunctionResult {
        ty: vec_type,
        binding: Some(Binding::Location {
            location: 0,
            interpolation: None,
            sampling: None,
        }),
    });

    let entry_point = EntryPoint {
        name: "main".to_string(),
        stage: ShaderStage::Compute,
        early_depth_test: None,
        workgroup_size: [1, 1, 1],
        function,
    };
    module.entry_points.push(entry_point);

    println!("Validating module...");
    let mut validator = Validator::new(ValidationFlags::all());
    let module_info = validator.validate(&module).unwrap();

    println!("Writing module...");
    // let mut str = String::new();
    // let mut writer = wgsl::Writer::new(&mut str);
    // writer.write(&module, &module_info).unwrap();
    // writer.finish();

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(Path::new("output.spv"))
        .unwrap();
    let mut words = vec![];
    let options = spv::Options::default();
    let mut writer = spv::Writer::new(&options).unwrap();
    writer.write(&module, &module_info, &mut words);
    let words_u8 = unsafe {
        slice::from_raw_parts(
            words.as_ptr() as *const u8,
            words.len() * mem::size_of::<u32>(),
        )
    };
    file.write(words_u8);

    // println!("Module:\n{}", &str);
    println!("Done.");
}
