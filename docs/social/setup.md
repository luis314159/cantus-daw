# Las cuentas de Cantus, paso a paso

*Ronda 42, 2026-11-22. Para Luis. Todo lo que hay que escribir en cada perfil está en
[`accounts.md`](accounts.md) (en inglés y en español, con los caracteres contados); la subida
del video de lanzamiento, en [`youtube-launch.md`](youtube-launch.md); las imágenes, en
`F:\projects\video-work\social\` (`sheet.png` las muestra todas juntas, con sus recortes).
Ningún agente crea cuentas ni publica: estos pasos son tuyos. Calcula una hora y media
para todo, más la espera de las verificaciones.*

**Lo que necesitas a la mano:** tu acceso a Cloudflare (donde está cantusdaw.com), tu cuenta
personal de Google, tu teléfono, un gestor de contraseñas (Bitwarden, 1Password o el de tu
navegador) y una app de autenticación (Google Authenticator, Microsoft Authenticator o la del
gestor).

**El orden importa:** primero el correo (sin él no llegan las verificaciones), luego proteger
Cloudflare (quien controla el dominio controla el correo, y con el correo se recupera
cualquier cuenta), luego YouTube, luego reservar el nombre en las demás el mismo día.

## 1. Protege Cloudflare primero

1. Entra a <https://dash.cloudflare.com> ▸ tu perfil (arriba a la derecha) ▸ **My Profile ▸
   Authentication**.
2. Activa **Two-Factor Authentication** con la app de autenticación (o una llave de
   seguridad). Guarda los **códigos de respaldo** (ver §6).
3. Si alguien más tiene acceso a la cuenta de Cloudflare, revisa **Members** y deja solo a
   quien debe estar.

## 2. El correo hola@cantusdaw.com (Cloudflare Email Routing)

Email Routing **recibe** el correo de hola@cantusdaw.com y lo **reenvía** a tu correo de
siempre. No envía correo como hola@ (para eso haría falta otro servicio), pero para crear y
recuperar cuentas basta con recibir.

1. Cloudflare ▸ el sitio **cantusdaw.com** ▸ **Email ▸ Email Routing ▸ Get started** (o
   *Enable Email Routing*).
2. Cloudflare propone añadir sus registros **MX** y un **TXT (SPF)**: acepta (**Add records
   and enable**). Si el dominio ya tuviera otros MX (otro servicio de correo), detente y
   avísame: no deben convivir.
3. **Destination addresses ▸ Add destination address**: tu correo personal. Llega un correo
   de Cloudflare: ábrelo y verifica.
4. **Routing rules ▸ Custom addresses ▸ Create address**: `hola` ▸ acción *Send to an email*
   ▸ tu correo personal ▸ **Save**.
5. Deja **Catch-all** desactivado (así no llega basura a cualquier@cantusdaw.com).
6. Prueba: envía un correo a hola@cantusdaw.com desde otra cuenta y comprueba que llega.

En tu correo personal, crea un filtro que etiquete lo que llega para hola@ (por ejemplo,
etiqueta *Cantus*), para que no se pierda entre lo demás.

## 3. El canal de YouTube como cuenta de marca

Una **cuenta de marca** (Brand Account) es un canal que no es tu perfil personal: puede tener
varios propietarios y administradores, y el canal sigue siendo tuyo aunque un día alguien más
te ayude a llevarlo. YouTube la crea sola cuando haces un canal nuevo desde tu cuenta.

1. Entra a YouTube con **tu cuenta personal de Google**. Antes, revisa que esa cuenta tenga
   la verificación en dos pasos activa (<https://myaccount.google.com/security>) y, mejor
   aún, una **llave de acceso (passkey)**.
2. Ve a <https://www.youtube.com/account> ▸ **Add or manage your channel(s)** (o
   *Agregar o administrar tus canales*) ▸ **Create a channel** (*Crear un canal*).
3. Nombre: **Cantus DAW**. Identificador (handle): **@cantusdaw**. Foto: `avatar-800.png`.
   ▸ **Create channel**.
4. YouTube Studio ▸ **Customization** (*Personalización*):
   - **Branding**: foto `avatar-800.png`; banner `youtube-banner.jpg`; marca de agua
     `youtube-watermark.png`, mostrada durante **todo el video**.
   - **Basic info**: la descripción en inglés de `accounts.md`; **Add language** ▸ Español,
     con la descripción en español; los enlaces (cantusdaw.com, el código, la comunidad);
     el correo de contacto **hola@cantusdaw.com**.
   - **Layout**: tráiler del canal y video destacado, cuando el video esté publicado
     (`youtube-launch.md`).
5. YouTube Studio ▸ **Settings ▸ Channel**: país **México**; palabras clave de
   `accounts.md`. **Settings ▸ Upload defaults**: categoría *Science & Technology*,
   licencia estándar, comentarios con revisión de los posiblemente inapropiados.
6. **Settings ▸ Permissions**: aquí se invita después a otras personas como administradores
   o editores, sin darles tu contraseña. Tú quedas como **propietario**. (Los propietarios de
   la cuenta de marca también se ven en <https://myaccount.google.com/brandaccounts>.)
7. Crea las cuatro listas (playlists) de `accounts.md`, aunque estén vacías: *Launch /
   Lanzamiento*, *Instruments / Instrumentos*, *Effects / Efectos*, *Tutorials / Tutoriales*.

## 4. Las verificaciones que pide YouTube

- **Verificación por teléfono** (funciones *intermedias*): miniaturas personalizadas, videos
  de más de 15 minutos. Ve a <https://www.youtube.com/verify>, elige mensaje de texto o
  llamada y escribe el código. Hazlo **antes** de subir el video: sin ella no se puede poner
  la miniatura. Un mismo número sirve para dos canales al año, como máximo.
- **Funciones avanzadas** (YouTube Studio ▸ Settings ▸ Channel ▸ **Feature eligibility**):
  enlaces que se pueden pulsar en las descripciones de los videos, audio en varios idiomas
  en un mismo video, entre otras. Un canal nuevo las obtiene con el tiempo (historial del
  canal) o al verificar con una **identificación oficial** o un **video de tu cara**, si
  YouTube te ofrece esa opción. Es tu decisión: Google guarda la identificación un tiempo
  según sus reglas. Mientras no las tengas, las direcciones de las descripciones se leen pero
  quizá no se puedan pulsar; el video funciona igual.

## 5. Reserva @cantusdaw en las demás, el mismo día

Usa **hola@cantusdaw.com** como correo de cada cuenta y una contraseña distinta y larga
generada por el gestor. Si **@cantusdaw** ya está tomado en alguna, no improvises: anota la
plataforma y usa provisionalmente **@cantus_daw** (o **@cantusdaw_app** donde no se permita
el guion bajo), y avísame para ajustar el kit.

| Plataforma | Dónde | Notas |
|---|---|---|
| **Instagram** | la app ▸ tu perfil ▸ el nombre arriba ▸ **Agregar cuenta ▸ Crear cuenta nueva** | usuario `cantusdaw`, nombre *Cantus DAW*. Luego **Configuración ▸ Tipo de cuenta ▸ Cambiar a cuenta profesional ▸ Creador**, categoría *Software* (o la más cercana). Vincularla a tu Centro de cuentas personal es opcional: permite compartir entre cuentas, pero mezcla tus datos con los de Cantus. |
| **TikTok** | la app ▸ Perfil ▸ **Cambiar de cuenta ▸ Agregar cuenta** con correo | usuario `cantusdaw`, nombre *Cantus DAW*. **Configuración ▸ Cuenta ▸ Cambiar a cuenta de empresa**, categoría de software o la más cercana: así se puede poner el enlace en la biografía antes de tener 1 000 seguidores (según reportes, no verificado). |
| **X** | <https://x.com> ▸ crear cuenta con correo | usuario `cantusdaw`, nombre *Cantus DAW*. Ubicación *Mexico*, sitio web `https://cantusdaw.com`. |
| **Bluesky** | <https://bsky.app> ▸ crear cuenta (servidor *Bluesky Social*) | primero `cantusdaw.bsky.social`; luego el dominio como identificador, abajo. |
| **Mastodon** | <https://mastodon.social/auth/sign_up> | usuario `cantusdaw`, nombre *Cantus DAW*: queda **@cantusdaw@mastodon.social**. |

**Bluesky con el dominio como nombre (@cantusdaw.com)** — demuestra que la cuenta es la del
sitio, y nadie puede imitarla:

1. Bluesky ▸ **Settings ▸ Account ▸ Handle ▸ I have my own domain**. Escribe
   `cantusdaw.com`. La app muestra un registro TXT: host `_atproto`, valor `did=did:plc:…`.
2. Cloudflare ▸ cantusdaw.com ▸ **DNS ▸ Records ▸ Add record**: tipo **TXT**, nombre
   `_atproto`, contenido el valor `did=did:plc:…` tal cual ▸ **Save**.
3. De vuelta en Bluesky ▸ **Verify DNS Record** ▸ **Update to cantusdaw.com**.

## 6. Dos factores y códigos de recuperación, en todas

En cada cuenta (Cloudflare, Google, Instagram, TikTok, X, Bluesky, Mastodon):

- Activa la **verificación en dos pasos con app de autenticación** (o llave de acceso), no
  por SMS cuando haya opción: un número de teléfono se puede robar con un cambio de SIM.
  - Instagram: Centro de cuentas ▸ Contraseña y seguridad ▸ Autenticación en dos pasos.
  - TikTok: Configuración ▸ Seguridad ▸ Verificación en dos pasos.
  - X: Configuración ▸ Seguridad y acceso a la cuenta ▸ Seguridad ▸ Autenticación en dos
    fases ▸ App de autenticación.
  - Bluesky: Settings ▸ Account ▸ **Two-factor authentication** (por correo; el correo es
    hola@, protegido por Cloudflare y tu correo personal). Las **App Passwords** de
    Bluesky son para apps de terceros: nunca des la contraseña principal.
  - Mastodon: Preferencias ▸ Cuenta ▸ **Autenticación de dos factores**.
- Guarda los **códigos de recuperación** de cada una:
  1. en el gestor de contraseñas, en la entrada de esa cuenta;
  2. y una copia impresa, en un sobre en un lugar seguro de tu casa.

  Nunca en el repositorio, ni en un correo, ni en una nota del teléfono sin cifrar, ni en la
  memoria de Claude.
- En el gestor, una carpeta **Cantus** con las siete entradas: así, si un día otra persona
  ayuda con las redes, se le comparte esa carpeta (o se le invita como administrador donde
  exista), nunca tus cuentas personales.

## 7. Llena cada perfil

Con `accounts.md` abierto, copia en cada plataforma: nombre, biografía (en el idioma que
elijas: la tabla recomienda cuál), enlace, categoría, foto y portada. Los archivos:

| Plataforma | Foto | Portada |
|---|---|---|
| YouTube | `avatar-800.png` | `youtube-banner.jpg`; marca de agua `youtube-watermark.png` |
| Instagram, TikTok | `avatar-1080.png` | — |
| Bluesky | `avatar-1000.jpg` | `bluesky-banner-en.jpg` (o `-es`) |
| Mastodon, X | `avatar-400.png` | `header-en-1500x500.png` (o `-es`) |

En **Mastodon**, llena los cuatro campos del perfil (sitio, código, descarga, comunidad). El
del sitio se pondrá en verde cuando cantusdaw.com tenga el enlace de vuelta (§9).

Después, mira cada perfil desde el teléfono y desde la computadora: que la foto se vea
completa en el círculo y que la portada no tape nada con la foto. `sheet.png` muestra cómo
deberían verse.

## 8. El video de lanzamiento

Sigue [`youtube-launch.md`](youtube-launch.md): se sube como **privado**, se revisa, y se
publica **solo después de beta.17** y del sitio que muestra Fides y Nitor. Las publicaciones
fijadas de las demás redes (en `accounts.md`) salen el mismo día, cada una con su archivo.

## 9. Mándame estos enlaces

Cuando las cuentas existan, envíame (o pega en la sesión) estas direcciones exactas, para que
el integrador las ponga en el pie del sitio y en la sección *Community* del README (eso lo
hace el integrador después, no este kit):

```text
YouTube:   https://www.youtube.com/@cantusdaw
Instagram: https://www.instagram.com/cantusdaw/
TikTok:    https://www.tiktok.com/@cantusdaw
X:         https://x.com/cantusdaw
Bluesky:   https://bsky.app/profile/cantusdaw.com   (o cantusdaw.bsky.social)
Mastodon:  https://mastodon.social/@cantusdaw
Video EN:  https://youtu.be/…
Video ES:  https://youtu.be/…
```

Con el de Mastodon, el sitio llevará un enlace con `rel="me"` a tu perfil: es lo que pone en
verde el campo *Website*. Con los de los videos, el README puede enlazar el video donde hoy
dice que irá (`README.md`, el comentario *TODO(round 41, video)*), y la página del sitio
puede ponerlo junto a la descarga.

Si algún nombre cambió (porque @cantusdaw estaba tomado), dímelo también: hay que cambiarlo
en `accounts.md` y en las imágenes que lo mencionen (hoy ninguna lo menciona; solo
cantusdaw.com).
