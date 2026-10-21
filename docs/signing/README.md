# Firma de código con SignPath Foundation — qué se verificó y qué falta

*Para el dueño, en español (2026-10-13, ronda 26). Los borradores están en inglés porque van
a SignPath y a la web: [`signpath-application.md`](signpath-application.md) (la solicitud,
campo por campo), [`code-signing-policy.md`](code-signing-policy.md) (la página de
política de firma) y [`signpath-release.draft.yml`](signpath-release.draft.yml) (cómo
firmaría `release.yml`; inerte). Nada de esto está publicado ni enviado.*

## 1. En resumen

- **Hoy no se puede enviar.** Tres condiciones fallan y no dependen del código: el
  repositorio es **privado**, **no hay una versión publicada** y no hay **reputación
  pública** (SignPath la pide para programas ejecutables). Se arreglan haciendo público el
  repositorio, publicando `v0.1.0-beta.1` sin firmar y esperando unas semanas de uso.
- **Una condición puede hacer que te rechacen: el CLA.** SignPath exige *"una licencia
  OSI sin licenciamiento dual comercial"*. Nuestro `CLA.md` (§2.1 y §3) y el ADR 0047 §3
  **reservan explícitamente la posibilidad de vender una licencia comercial** de Cantus
  además de la libre. Hoy nadie la vende, pero está escrito. Tienes que decidir (§3).
- Tu plan de monetización (`docs/monetize.md`, en tu copia) **no** incluye vender
  licencias del software: donaciones, paquetes de sonidos, patrocinios, subvenciones,
  servicios e IA alojada opcional. Todo eso es compatible con SignPath. Lo único que
  choca es la frase del CLA.
- Lo demás pasa o se arregla con trabajo nuestro: los metadatos de versión y las fuentes
  y la lista de certificados con licencias no OSI ya se arreglaron en esta ronda; faltan la
  página de política y el flujo de firma en dos pasos.

## 2. Las condiciones, una por una

Fuente: <https://signpath.org/terms> («Code of conduct», marcado *Draft*, sin fecha),
leído el 2026-10-13; el formulario de <https://signpath.org/apply> leído de su definición.
✅ pasa · ❌ falla hoy · ⚠️ dudoso o pendiente de ti.

| # | Condición (sus palabras, resumidas) | Estado | Evidencia y qué hacer |
|---|---|---|---|
| 1 | **Sin malware** ni programas no deseados | ✅ | Código propio; sin instaladores de terceros empaquetados (FlexASIO es opcional, desmarcado, se baja del autor al instalar). |
| 2 | **Licencia OSI** en todos los componentes | ✅ (arreglado en la ronda 26) | App GPL-3.0-or-later, motor MPL-2.0 (ADR 0047, `Cargo.toml` de cada crate); dependencias: 290+ crates MIT/Apache/BSD/ISC/Zlib/BSL y LAME bajo LGPL-2.0-or-later (LAME 3.100; los metadatos del crate dicen LGPL-3.0) — todas OSI. **Dudas menores:** las fuentes de respaldo de egui (`epaint_default_fonts`): *Ubuntu Font Licence 1.0* y la licencia *Bitstream Vera* de Hack **no están aprobadas por la OSI** (son libres, pero no OSI); y `webpki-roots` (la lista de certificados raíz de Mozilla) es *CDLA-Permissive-2.0*, una licencia de datos. **Hecho (ronda 26):** las fuentes de egui ya no van dentro (ninguna cadena de fuentes las usaba; un test comprueba que cada carácter de la interfaz tiene glifo, y 85 capturas son idénticas byte a byte); `webpki-roots` tampoco (`cantus-content` toma los certificados del sistema). Quedan IBM Plex (OFL-1.1) y emoji-icon-font (MIT), ambas OSI. |
| 3 | **Sin licenciamiento dual comercial** | ⚠️ **riesgo de rechazo** | `CLA.md` §2.1: el contribuidor permite *"license it under other terms, including commercial ones"*; §3: *"The Steward may also offer Cantus under other terms (for example a commercial licence that helps fund the project)"*; ADR 0047 §3: *"keeps open a commercial licence beside the free one (BACKLOG §6f), as Qt and others do"*. BACKLOG §6f y `docs/monetize.md` no mencionan vender licencias. Ver §3, decisión 1. |
| 4 | **Sin código propietario** (sobre todo del propio mantenedor); se permiten bibliotecas del sistema | ✅ | **ASIO SDK**: Steinberg lo licencia *"under the terms of the Steinberg ASIO License, or alternatively under the terms of the General Public License (GPL) Version 3"* (`ASIOSDK/LICENSE.txt` 2.3.4, 2025); usamos la opción GPL-3 y no se guarda en el repositorio (`xtask asio-sdk` lo baja con hash fijo); se compilan **solo los tres archivos del lado del anfitrión**, de Steinberg y bajo esa licencia: `common/asio.cpp`, `host/asiodrivers.cpp`, `host/pc/asiolist.cpp` (`third_party/asio-sys/build.rs`, ronda 26). Antes se compilaban también los del lado del controlador, que un anfitrión no usa, entre ellos `combase.cpp` y `dllentry.cpp` de Microsoft ("All Rights Reserved", sin licencia); ya no. Comprobado: la app enlaza con ASIO, `--audio-drivers` dice `WASAPI, ASIO`, y el ejemplo `enumerate` carga ASIO4ALL y FL Studio ASIO. **VST3**: SDK MIT (crate `vst3`). **CLAP**: MIT. **RVR1960**: bloqueada — `verse::RVR1960_PERMITTED = false` hace que el texto no entre al programa: los ejecutables de prueba del 12–13 oct (`F:\projects\daw-r25\builds\night`, `cables`) no contienen la frase propia de la RVR1960 de Colosenses 3:23, el del 10 oct sí. **Contenido**: los paquetes de sonidos CC0 se bajan aparte, no van firmados ni dentro del instalador. |
| 5 | **Mantenido** activamente | ✅ | Commits diarios, 26 rondas desde septiembre de 2026. |
| 6 | **Ya publicado** en la forma que se firmará | ❌ | No hay ninguna versión pública. Publicar `v0.1.0-beta.1` sin firmar primero. |
| 7 | **Documentado** en la página de descarga | ✅ cuando sea público | cantusdaw.com describe el programa; las notas de la versión también. |
| 8 | Firmar **solo proyectos propios**: el equipo es dueño del repositorio | ✅ | `github.com/luis314159/cantus-daw`, tuyo. |
| 9 | Firmar **solo binarios propios**; mantener todo el código y los scripts | ✅ (⚠️ menor) | Todos los programas y plug-ins salen de nuestro código. `third_party/cpal` y `asio-sys` son copias parcheadas (Apache-2.0) que se compilan *dentro* de `cantus.exe`, como cualquier otra dependencia; no firmamos binarios de terceros. Si preguntan, `third_party/README.md` dice qué cambiamos y por qué. |
| 10 | **Sin herramientas de hacking** | ✅ | No aplica. |
| 11 | **Privacidad**: si se transfieren datos del usuario a sistemas que él no eligió, política, mostrarla al instalar y opción para desactivar | ✅ (completo en la ronda 26) | Cantus no recoge datos del usuario. La única petición automática es la comprobación diaria de versiones (sin datos; se apaga en *Ajustes ▸ Actualizaciones*), documentada en `PRIVACY.md`. **Y ahora el instalador tiene una página *Privacidad*** que resume qué envía Cantus y ofrece *Buscar actualizaciones una vez al día* (marcada salvo que Cantus ya la tenga apagada); Cantus aplica la elección en su siguiente inicio. Las tres partes de la condición se cumplen aunque SignPath la considere transferencia. |
| 12 | **Avisar cambios en el sistema** | ✅ | Instalación por usuario sin administrador; la asociación `.cantus` es un componente visible; FlexASIO pide UAC y está desmarcado. |
| 13 | **Desinstalación** | ✅ | `uninstall.exe` en *Aplicaciones*, borra exactamente lo instalado (un test lo comprueba). |
| 14 | **MFA** de todo el equipo en GitHub y SignPath | ⚠️ tú | Confirma que tu cuenta de GitHub tiene la verificación en dos pasos (GitHub ya la exige a quien sube código); actívala en SignPath al crear la cuenta. |
| 15 | **Roles**: autores, revisores (todo PR de alguien de fuera revisado), aprobadores | ✅ | Eres los tres. Escrito en la página de política. Si entra alguien, se añade. |
| 16 | **Política de firma** en la web: el término «Code signing policy» en la página principal y en la de descargas; la frase *"Free code signing provided by SignPath.io, certificate by SignPath Foundation"*; roles; privacidad | ⚠️ borrador | `code-signing-policy.md`, listo para ser página de la web cuando SignPath acepte (no antes: sería falso). |
| 17 | **Metadatos**: nombre de producto y versión en todos los binarios firmados | ✅ (arreglado en la ronda 26) | `cantus.exe`, los `cantus-x86-64-v*.exe` y el instalador los tienen (`winresource`, `VIAddVersionKey`). **Ahora también** `cantus-scan.exe` y cada plug-in `.clap`/`.vst3` (`build.rs` con `winresource` en `cantus-host` y `cantus-plugins`), y `xtask package` se niega a publicar si un archivo a firmar no los tiene — el `uninstall.exe` de NSIS incluido (lleva los datos del instalador). |
| 18 | **Compilación automatizada y verificable**, todos los trabajos en runners de GitHub; **aprobación manual** de cada versión | ✅ (cambio pendiente) | `release.yml` compila en `windows-latest`/`ubuntu-latest` desde la etiqueta, con NSIS y el ASIO SDK verificados por SHA-256. Falta cambiar la firma al flujo de SignPath: dos solicitudes por versión (programas, luego instalador) — `signpath-release.draft.yml` y ADR 0048 §8. **El trabajo que envía a SignPath, y todo lo que compila lo que se firma, corre sin `rust-cache` ni ningún caché restaurado** (SignPath pide que lo firmado salga de una compilación limpia del código): hoy `release.yml` restaura `rust-cache` en `test` y `package`; el borrador lo quita del trabajo de empaquetado. |
| 19 | **El editor es SignPath Foundation** | ⚠️ tú | Windows mostrará «SignPath Foundation», no tu nombre. Ver §5 sobre el actualizador. |

## 3. Lo que tienes que decidir

1. **El CLA y la licencia comercial.** Tres caminos:
   - **(a) Quitar la licencia comercial del CLA** (recomendado si eliges SignPath): en
     §2.1 limitar la relicencia a licencias aprobadas por la OSI (así se conserva poder
     pasar a GPL-4, AGPL o a una organización Cantus), y en §3 borrar el ejemplo de la
     licencia comercial; corregir ADR 0047 §3 y la memoria. Tu plan de monetización no la
     necesita. Mientras no haya contribuciones de fuera, el cambio no afecta a nadie.
   - **(b) Preguntar a SignPath antes** (support@signpath.io): si una cláusula de CLA que
     *permite* relicenciar, sin ninguna licencia comercial ofrecida, cuenta como
     «licenciamiento dual comercial». Su página dice que no discuten política y que la
     decisión es suya; la respuesta puede ser «no».
   - **(c) Mantener el CLA como está y no usar SignPath**: Certum Open Source (≈ 30–70 €/año,
     sale tu nombre) o Azure Trusted Signing si eres elegible desde México
     (`docs/round20/signing.md`).
2. **SignPath o Certum.** SignPath es gratis y firma en GitHub Actions, pero el editor es
   «SignPath Foundation», hay que pasar su revisión (reputación) y cada versión espera tu
   aprobación. Certum pone tu nombre y no tiene estas condiciones, pero cuesta y firmar en
   CI es incómodo (tarjeta virtual en una máquina con sesión abierta).
3. **El nombre del editor** (`CANTUS_PUBLISHER`, se fija una vez): con SignPath sería
   exactamente `SignPath Foundation`.
4. **Datos del formulario** marcados `[OWNER: …]`: nombre y apellidos como en tu
   identificación, el correo para las aprobaciones, «cómo nos conociste».

## 4. Los pasos, en orden

1. Hacer público el repositorio (limpieza del historial: `docs/round26/public.md`).
2. Publicar `v0.1.0-beta.1` **sin firmar** (la etiqueta; `release.yml` hace el resto).
3. Decidir lo del CLA (§3.1) y, si es (a), cambiarlo antes de la solicitud.
4. ~~Metadatos de versión en `cantus-scan.exe` y los plug-ins (condición 17).~~ Hecho.
5. Unas semanas de uso; anotar descargas, estrellas, menciones (campo *Reputation*).
6. Enviar la solicitud (`signpath-application.md`).
7. **Tras la aprobación**: crear en SignPath el proyecto `cantus`, enlazar el *trusted
   build system* GitHub.com, instalar la GitHub App de SignPath, crear la política
   `release-signing` y las dos configuraciones de artefacto; en GitHub, el secreto
   `SIGNPATH_API_TOKEN` (un usuario de CI que solo pueda *enviar*) y la variable
   `SIGNPATH_ORGANIZATION_ID`; cambiar `release.yml` como en el borrador (`package
   --stage-only` / `--from-stage` y `xtask sums` ya existen y están probados); publicar la página de política y los enlaces
   «Code signing policy» en la web y en las notas de cada versión; poner
   `CANTUS_PUBLISHER = SignPath Foundation`.
8. La primera versión firmada: dos aprobaciones tuyas en SignPath (programas, instalador).
   La gente con la beta sin firmar actualiza normalmente (la beta sin firma no exige editor).

## 5. Una consecuencia para el actualizador

El actualizador de Cantus, si el programa se compiló con `CANTUS_PUBLISHER`, exige que la
actualización esté firmada por **ese mismo nombre** (ADR 0032 §5). Con SignPath ese nombre
es «SignPath Foundation», **el mismo para todos los proyectos que firma la fundación**: la
comprobación del editor ya no distingue a Cantus de otro proyecto firmado por ellos. La
seguridad no baja mucho, porque el actualizador además exige el SHA-256 que GitHub calcula
al subir el archivo y la línea de `SHA256SUMS.txt` de **nuestra** versión en **nuestro**
repositorio (alguien tendría que controlar nuestras versiones de GitHub para engañarlo).
Pero conviene saberlo: con Certum (tu nombre) el pin sería más fuerte. Si se elige
SignPath, una mejora posible después: comprobar también la huella del certificado o el
nombre del producto en la versión firmada.
