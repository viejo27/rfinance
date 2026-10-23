# SECURITY.md — rfinance_app

Guía operativa de seguridad. El análisis completo vive en
`rfinance_app-threat-model.md`; este archivo es el runbook de lo que hay
que hacer (y qué hacer cuando sale mal).

## Postura CSRF (documentada a propósito)

El backend **no lleva tokens CSRF** y no hace falta, mientras se cumplan
estas tres condiciones:

1. El cookie de sesión nace con `SameSite=Lax` (default de actix-session
   0.10): un POST cruzado desde otro sitio no lo lleva.
2. Todo endpoint mutante toma `web::Json`: el extractor exige
   `Content-Type: application/json`, que dispara preflight CORS en un
   navegador; la app no publica CORS, así que el preflight cae.
3. No hay endpoints mutantes GET ni handlers que acepten cuerpo de
   formulario.

**Regla de repo:** toda ruta nova mutante (`POST`/`PATCH`/`DELETE`) debe usar
`web::Json`. Si algún día hace falta un endpoint que acepte form-encoded (p.
ej. OAuth redirects), en ese momento se revisa esta sección y se evalúa un
middleware de `Sec-Fetch-Site`.

## Runbook de incidentes

### Rotación de `SESSION_KEY` (cuando puede haber cookie forjada o robada)

La sesión completa vive cifrada+firmada en el cookie con esta clave. Si hay
sospecha de fuga (backup filtrado, máquina de dev comprometida, ex-miembro
del equipo con `.env`), la única revocación real es rotar:

1. Generar clave nueva: `openssl rand -hex 32`
2. Reemplazar `SESSION_KEY` en el `.env` del servidor.
3. Reiniciar el proceso: **todas las sesiones quedan invalidadas**
   (usuarios debian reingresar). No hay otro efecto sobre los datos.
4. Si se sospecha explotación activa: revisar el `AUDIT`/registrar accesos
   posteriores a la ventana, en particular `GET /admin`.

### Administrador y sesiones

- El rol `admin` viaja firmado en la cookie, **pero** `GET /admin`
  revalida el rol contra `user_data` en la base (`src/routes/web/admin.rs`):
  bajar el rol en la DB tiene efecto inmediato para ese endpoint.
- `ensure_admin_user` re-eleva al email de `ADMIN_USER` en cada arranque.

### Contraseñas

- Política server-side: mínimo 10 caracteres, máximo 72 bytes
  (`src/routes/api/register.rs`).
- bcrypt cost 12; sin lockout por cuenta, parte del peso lo lleva el rate
  limit (abajo).

## Rate limiting (dos capas)

**Capa 1 — in-app** (`actix-governor`, `src/main.rs`): por IP real
(`X-Real-IP`, que **el proxy debe setear**):

| Endpoint | Límite | Burst |
|---|---|---|
| `POST /api/login` | 5/min | 5 |
| `POST /api/register` | 3/min | 3 |

Sin el header `X-Real-IP` desde el proxy, **todo el mundo compartiría la
misma cubeta** (ver `RealIpKeyExtractor` src/main.rs).

**Capa 2 — proxy** (`deploy/nginx-rfinance.example.conf`): `limit_req` con
los mismos valores sobre `/api/login` y `/api/register`. Copiar el ejemplo a
nginx, ajustar `server_name`/certificados y el include de params.

## Checklist de despliegue

- [ ] `APP_ENV=production` en el `.env` del servidor (activa `Secure` en el
  cookie de sesión).
- [ ] `SESSION_KEY` real generada con `openssl rand -hex 32`: ninguna
  instalación debería correr con la de ejemplo.
- [ ] TLS en el proxy, app en loopback (default de `BIND_ADDR`,
  `127.0.0.1:8080`). Abrir `BIND_ADDR=0.0.0.0:8080` sin TLS es solo para
  pruebas en red de confianza: los cookies de sesión son replayables si se
  sniffean.
- [ ] Nginx seteando `X-Real-IP` (`deploy/rfinance_proxy_params.example.conf`).
- [ ] Logs del proceso enviados a un lugar conservado (off-host idealmente):
  access logs (`Logger`) y líneas `AUDIT` de mutaciones financieras.
- [ ] Backups de PostgreSQL ubicados y con la misma higiene de secretos que
  los servidores (los datos son financieros).
- [ ] Auditar dónde hay copias de `.env`: el `SESSION_KEY`/`DATABASE_URL`
  filtrados son el incidente único más grave de este servicio (TM-002).

## Monitoreo sugerido (con evidencia que ya produce el app)

- **Fallos de login por minuto** por IP y email: el 401 del login y el 429
  del governor ya aparecen en el access log; alertar en patrones.
- **Filas por usuario**: crecimiento anómalo de `transactions` (el API
  capotó cuotas en 600 y rows de materialize por llamada en 120).
- **Tasa de 5xx / panics**: señales de DoS por error (TM-008).
- **Tráfico anómalo en `GET /admin`**: cualquiera que toca esa ruta siendo
  usuario común merece revisión.

## Limitaciones conocidas (aceptadas, ver threat model)

- Logout solo borra la copia local del cookie: **no hay revocación
  server-side** por sesión (store client-side). El desalojo masivo es la
  rotación de `SESSION_KEY`. Cambiarlo a un session store server-side es el
  único escalón estructural que queda abierto.
- El campo `ip`/`user_agent` de `user_data` existe pero ninguna ruta lo
  escribe: no colectamos ese dato hoy.
- El mensaje de `/api/register` confirma si un email está registrado
  ("El email ya está registrado"): decisión de producto (UX del alta). El
  ataque scriptado queda contenido por rate limit, y el login ya no filtra
  qué emails existen por timing (hash bcrypt dummy en
  `src/routes/api/login.rs`).
