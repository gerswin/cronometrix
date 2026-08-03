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

/// Fix round 1 (Critical): una barra final después del token no puede hacer
/// que el token sobreviva en texto plano. Una normalización de proxy, un
/// firmware con manía de barra final, o un retry pueden producir esta forma
/// -- el 404 ocurre en el enrutador, pero el span ya se construyó antes.
#[test]
fn push_token_with_trailing_slash_is_still_redacted() {
    let redacted = redact_path("/api/v1/devices/dev-123/push/s3cr3t-t0ken-value/");
    assert!(!redacted.contains("s3cr3t"));
    assert_eq!(redacted, "/api/v1/devices/dev-123/push/[redacted]/");
}

/// Fix round 1 (Critical): un segmento extra después del token tampoco puede
/// dejar el token en claro. Solo el primer segmento (el token) se redacta;
/// cualquier segmento posterior se conserva intacto para no perder
/// observabilidad del resto de la ruta.
#[test]
fn push_token_with_trailing_segment_is_still_redacted() {
    let redacted = redact_path("/api/v1/devices/dev-123/push/s3cr3t-t0ken-value/extra");
    assert!(!redacted.contains("s3cr3t"));
    assert_eq!(redacted, "/api/v1/devices/dev-123/push/[redacted]/extra");
}
