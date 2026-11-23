use breadb_core::{
    error::DbError,
    schema::{Column, Schema, Tuple},
    types::{DataType, Value},
};

#[test]
fn test_tuple_roundtrip_serialization() {
    let schema = Schema::new(vec![
        Column::new("id", DataType::Integer).primary_key(),
        Column::new("username", DataType::Text).not_null(),
        Column::new("score", DataType::Float),
        Column::new("active", DataType::Boolean),
    ])
    .unwrap();

    let tuple = Tuple::new(vec![
        Value::Integer(42),
        Value::Text("alice".into()),
        Value::Null,
        Value::Boolean(true),
    ]);

    assert!(schema.validate_values(&tuple.values).is_ok());

    let bytes = tuple.to_bytes().unwrap();
    let decoded = Tuple::from_bytes(&bytes).unwrap();
    assert_eq!(tuple.values, decoded.values);
}

#[test]
fn test_null_constraint_violation() {
    let schema = Schema::new(vec![Column::new("id", DataType::Integer).not_null()]).unwrap();
    let err = schema.validate_values(&[Value::Null]).unwrap_err();
    assert!(matches!(err, DbError::NullConstraintViolation(_)));
}
