use cronometrix_api::http_trace::redact_path;

/// C-08: el token de push es un secreto de escritura. Nunca debe aparecer en
/// un span, y por tanto tampoco en un archivo de log ni en un agregador.
#[test]
fn push_token_is_redacted_from_the_path() {
    let redacted = redact_path("/api/v1/devices/dev-123/push/s3cr3t-t0ken-value");
    assert_eq!(redacted, "/api/v1/devices/dev-123/push/[redacted]");
    assert!(!redacted.contains("s3cr3t"));
}

/// La redacción no puede degradar la observabilidad del resto de la API.
#[test]
fn other_paths_are_left_untouched() {
    assert_eq!(redact_path("/api/v1/employees"), "/api/v1/employees");
    assert_eq!(
        redact_path("/api/v1/devices/dev-123/status"),
        "/api/v1/devices/dev-123/status"
    );
}

/// Un push sin token (ruta incompleta) no debe entrar al brazo de redacción y
/// tampoco romper.
#[test]
fn push_without_token_is_left_untouched() {
    assert_eq!(
        redact_path("/api/v1/devices/dev-123/push"),
        "/api/v1/devices/dev-123/push"
    );
}
