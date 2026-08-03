# Rotación de la clave de firma de licencias (C-06)

La clave privada `do-functions/test-keys/test_priv.pem` estuvo versionada y
corresponde a la pública que el backend compila. **Debe tratarse como
comprometida**, aunque no haya evidencia de uso indebido: sigue estando en la
historia de git y sigue siendo válida hasta que se rote.

## 1. Generar el par nuevo (fuera del repositorio)

```bash
umask 077
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:4096 -out ~/cronometrix-license-priv.pem
openssl pkey -in ~/cronometrix-license-priv.pem -pubout -out ~/cronometrix-license-pub.pem
```

La privada nunca entra al repositorio, ni a un `.env` versionado, ni a un
issue, ni a un chat.

## 2. Sustituir la pública que confía el backend

```bash
cp ~/cronometrix-license-pub.pem backend/src/license/pubkey.pem
```

Se compila con `include_str!`, así que **rotar exige recompilar y redesplegar**.

## 3. Cargar la privada en la función de firma

Colocarla como secreto de DigitalOcean Functions (nunca como archivo del
paquete) y verificar que `do-functions` la lee de entorno.

## 4. Reemitir las licencias vigentes

Toda licencia firmada con la clave vieja deja de verificar en cuanto se
despliegue el binario nuevo. Reemitir **antes** de desplegar, o los clientes
quedan fuera de servicio.

## 5. Verificar

```bash
openssl pkey -in ~/cronometrix-license-priv.pem -pubout | openssl sha256
openssl pkey -pubin -in backend/src/license/pubkey.pem -pubout | openssl sha256
```

Los dos SHA-256 deben coincidir entre sí y **ser distintos** de
`f0318260deb1672cb624314065c8bd42d394fd799a954580265ca3b19f3106fc`, que es el
de la clave comprometida.

## 6. Purgar la historia (coordinado, destructivo)

Borrarla del árbol no la saca de la historia. Requiere `git filter-repo` o
BFG y un push forzado, coordinando con todos los clones — incluidos los
worktrees activos. Mientras esto no se haga, la rotación de los pasos 1-5 es
lo que realmente protege: la clave vieja queda en la historia pero ya no
firma nada que el producto acepte.

## 7. Hallazgo resuelto: la misma clave también vivía en `backend/tests/fixtures/`

Durante la Tarea 4 (C-06) se confirmó que `backend/tests/fixtures/test_license_privkey.pem`
era **byte-idéntico** a `do-functions/test-keys/test_priv.pem` (la clave que
esta tarea retiró) y a la pública comprometida
(`f0318260deb1672cb624314065c8bd42d394fd799a954580265ca3b19f3106fc`). No se
resolvió en la primera pasada porque `backend/tests/license_tests.rs` y
`backend/tests/license_service_extra_test.rs` firmaban JWTs de prueba con esa
clave para verificarlos contra `verify_license_jwt`, que lee
`backend/src/license/pubkey.pem` embebido en tiempo de compilación
(`include_str!`, sin punto de inyección para pruebas).

**Resuelto en la ronda de corrección 1:** `verify_license_jwt` se dividió en
un envoltorio de producción (sin cambios de comportamiento, sigue usando
solo la clave embebida — nada en el camino de producción puede influir en
qué clave confía, ni por variable de entorno ni de ninguna otra forma en
tiempo de ejecución) y una función pura `verify_license_jwt_with_key(token,
key)` que recibe la clave como parámetro. El mismo patrón se replicó en
`load_and_validate_license`, `activate_license`, `try_renew` y
`renewal_task` (cada uno gana un gemelo `*_with_key`), porque las pruebas de
extremo a extremo de esas cuatro funciones también dependían, de forma
transitiva, de que la clave de prueba coincidiera con la de producción.
Las pruebas ahora generan un par RSA-2048 efímero por binario de prueba
(`rsa::RsaPrivateKey::new` vía `rand::thread_rng()`, ya transitivo por el
backend `rust_crypto` de `jsonwebtoken` — se declaró explícito en
`[dev-dependencies]`) y firman/verifican contra ese par, nunca contra la
clave de producción. `backend/tests/fixtures/test_license_{priv,pub}key.pem`
se eliminó del árbol.
