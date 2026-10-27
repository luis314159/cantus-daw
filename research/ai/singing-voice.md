# Investigación: generación de voz cantada controlada por MIDI

*La investigación del dueño (Luis Ángel Almazán), entregada el 2026-10-27 para el backlog
(`docs/BACKLOG-excellence.md` §7a). Se guarda tal como llegó; las capacidades, idiomas y
licencias que nombra deben confirmarse en la documentación vigente antes de decidir. Se
relaciona con §3e punto 10 (el coro de vocales por formantes, y el canto con letra
después).*

## 1. Objetivo

Investigar cómo generar voces cantadas mediante inteligencia artificial a partir de una
descripción vocal, notas MIDI, tempo y letra en uno o varios idiomas.

El objetivo a largo plazo es evaluar una posible integración en Cantus DAW. El sistema
debería permitir definir el timbre y el estilo de la voz, proporcionar una melodía y una
letra, y obtener audio cantado que respete las notas y el tiempo musical.

## 2. Requisitos funcionales

El sistema ideal debería aceptar:

- **Entrada musical:** MIDI, notas, tempo en BPM, compás, posición y duración de cada nota.
- **Perfil vocal:** timbre, registro, tipo de voz, estilo, expresividad y técnica vocal.
- **Letra:** texto por idioma y su asignación a notas o sílabas.
- **Fonética:** fonemas y pronunciación por idioma.
- **Renderizado:** generación de audio sincronizado con el proyecto y exportación a WAV.
- **Edición:** posibilidad de corregir sílabas, fonemas, duración, pronunciación y
  expresión.
- **Multilingüismo:** conservar, en lo posible, la melodía y el ritmo al adaptar la letra a
  diferentes idiomas.

## 3. Flujo de trabajo propuesto

1. Entrada musical: importar MIDI o introducir notas, tempo, compás y duración.
2. Perfil vocal: seleccionar una voz y definir registro, timbre, estilo y expresividad.
3. Letra y fonemas: introducir la letra, elegir el idioma y asignar sílabas a las notas.
4. Alineación temporal: determinar el inicio y duración de cada sílaba y fonema.
5. Síntesis: generar el audio cantado usando un motor compatible.
6. Validación: comprobar afinación, sincronización, inteligibilidad y naturalidad.
7. Exportación: renderizar un WAV alineado con el proyecto de la DAW.

## 4. Problemas técnicos principales

### 4.1 Afinación (pitch)

La voz debe seguir las notas MIDI de forma fiable, incluidos saltos de intervalo, notas
sostenidas y cambios de registro. También hay que definir cómo se permiten vibrato y
variaciones expresivas sin perder la nota objetivo.

### 4.2 Sincronización (timing)

El inicio de las sílabas y fonemas debe corresponder con la posición musical. Deben
probarse síncopas, silencios, notas largas, cambios de tempo y frases rápidas.

### 4.3 Pronunciación y fonética

Cada idioma utiliza fonemas y reglas de pronunciación diferentes. La traducción de una
letra no garantiza que la cantidad de sílabas, los acentos prosódicos o las vocales encajen
con la melodía.

### 4.4 Naturalidad

La síntesis debe manejar transiciones entre notas, respiraciones, consonantes, vocales
sostenidas, vibrato y expresión dinámica. La calidad puede variar significativamente entre
modelos y bancos vocales.

### 4.5 Multilingüismo

Conviene distinguir dos estrategias:

- **Adaptación automática:** traducir o adaptar la letra y repartir las sílabas entre las
  notas. Es más sencillo para el usuario, pero puede necesitar ajustes para respetar
  métrica y acentuación.
- **Edición fonética explícita:** permitir editar sílabas y fonemas manualmente. Da más
  control, aunque requiere una interfaz más especializada.

La propuesta es combinar ambas: adaptación automática como punto de partida y edición
fonética para corregir el resultado.

## 5. Tecnologías que conviene evaluar

### 5.1 ACE Studio

Referencia comercial para evaluar la experiencia de usuario y la calidad de las voces de
IA. Hay que comprobar qué idiomas admite cada modelo, qué controles ofrece y cuáles son las
condiciones de uso e integración.

- Documentación: https://docs.acestudio.ai/

### 5.2 OpenUtau

Editor de síntesis vocal que permite trabajar con notas, letras, fonemizadores y bancos
vocales compatibles. Es una opción útil para estudiar el flujo de trabajo de un editor de
canto.

- Repositorio: https://github.com/openutau/OpenUtau

### 5.3 DiffSinger

Familia de modelos de síntesis vocal cantada que puede generar audio condicionado por
información musical y fonética. La calidad, los idiomas, los requisitos y las licencias
dependen de la versión y de los modelos utilizados.

- Repositorio original: https://github.com/MoonInTheRiver/DiffSinger

### 5.4 Entrenamiento o adaptación de un modelo propio

Como posible fase futura, investigar modelos acústicos, vocoders, alineación fonética,
control de pitch y expresividad. Esta opción ofrece más flexibilidad, pero implica costes
de datos, entrenamiento, evaluación y mantenimiento.

**Recomendación inicial:** probar OpenUtau con un modelo compatible de DiffSinger y comparar
los resultados con ACE Studio antes de invertir en un modelo propio.

## 6. Representación de datos propuesta

Una representación estructurada podría incluir:

```json
{
  "tempo_bpm": 120,
  "time_signature": [4, 4],
  "voice_profile": {
    "voice_type": "alto",
    "timbre": "warm",
    "style": "pop",
    "expression": "moderate"
  },
  "lyrics": {
    "language": "es",
    "text": "Hola mundo"
  },
  "notes": [
    {
      "start_tick": 0,
      "duration_ticks": 480,
      "midi_pitch": 60,
      "syllable": "Ho",
      "phonemes": ["o"]
    },
    {
      "start_tick": 480,
      "duration_ticks": 480,
      "midi_pitch": 62,
      "syllable": "la",
      "phonemes": ["l", "a"]
    }
  ],
  "render_settings": {
    "sample_rate": 44100,
    "format": "wav"
  }
}
```

Este JSON es un esquema ilustrativo, no un formato estándar ni una garantía de
compatibilidad con un motor concreto. Las unidades de ticks deben definirse según la
resolución MIDI y el tempo del proyecto. En una implementación real, el modelo de datos
debería distinguir las notas musicales de la alineación fonética y permitir que una sílaba
se extienda por varias notas.

Campos relevantes:

- `tempo_bpm` y `time_signature`
- `notes`: pitch MIDI, posición inicial y duración
- `lyrics`: texto original y variantes por idioma
- `phonemes`: fonemas asociados a sílabas o segmentos temporales
- `voice_profile`: timbre, registro, estilo y expresividad
- `render_settings`: frecuencia de muestreo, formato y opciones de renderizado

## 7. Plan experimental

### Fase 1: prototipo mínimo

- Instalar OpenUtau.
- Elegir un banco vocal y un modelo de síntesis cuya licencia y compatibilidad estén
  verificadas.
- Importar una melodía MIDI breve.
- Asignar una letra y generar un WAV.
- Medir cuánto trabajo manual necesita el resultado.

### Fase 2: pruebas controladas

Usar la misma melodía a 80, 120 y 160 BPM. Incluir notas sostenidas, saltos de intervalo y
de octava, consonantes difíciles, silencios y respiraciones, síncopas y frases rápidas.

Evaluar: error de afinación, desviación temporal respecto al MIDI, inteligibilidad de la
letra, naturalidad de la voz, tiempo de renderizado, uso de RAM y GPU, y cantidad de
correcciones manuales necesarias.

### Fase 3: pruebas multilingües

Probar español e inglés primero, y después portugués, francés, japonés u otros idiomas
según la disponibilidad real de los modelos. Para cada idioma, revisar pronunciación,
acentuación, encaje silábico, consonantes, vocales sostenidas y legibilidad de la letra
cantada.

### Fase 4: integración con Cantus DAW

Empezar con renderizado offline, no con síntesis en tiempo real. Arquitectura inicial:

1. Cantus exporta o entrega los datos musicales y vocales.
2. Un adaptador convierte los datos al formato aceptado por el motor.
3. El motor genera el audio.
4. El adaptador valida el resultado y lo devuelve como WAV.
5. Cantus coloca el audio en la línea de tiempo con el desplazamiento correcto.

La integración debería aislar el motor detrás de una interfaz para que sea posible
sustituirlo sin rediseñar toda la DAW.

## 8. Licencias y distribución

Antes de integrar o redistribuir cualquier tecnología, revisar por separado: la licencia
del código fuente; la de los pesos del modelo; la de los bancos vocales; las licencias y
restricciones de los datasets de entrenamiento; los permisos para uso comercial y
redistribución; las restricciones de atribución, uso no comercial o trabajos derivados; y
las condiciones de uso de los servicios comerciales y sus APIs.

No asumir que la licencia del repositorio cubre todos los modelos, voces, vocoders o
recursos asociados. Guardar una ficha de licencia y procedencia para cada componente.

## 9. Criterios para elegir la solución

| Criterio | Pregunta de evaluación |
|---|---|
| Fidelidad al MIDI | ¿Respeta pitch y duración de las notas? |
| Timing | ¿Las sílabas comienzan donde se espera? |
| Calidad vocal | ¿Suena natural y expresiva? |
| Idiomas | ¿Qué idiomas admite realmente el modelo? |
| Control fonético | ¿Se pueden corregir sílabas y fonemas? |
| Hardware | ¿Funciona en CPU o requiere GPU? |
| Latencia | ¿Cuánto tarda en generar una frase? |
| Licencia | ¿Permite el uso previsto en Cantus? |
| Distribución | ¿Se pueden redistribuir código, pesos y voces? |
| Integración | ¿Puede automatizarse mediante CLI, API o biblioteca? |

## 10. Decisión recomendada

La primera meta no debería ser entrenar un modelo, sino demostrar que se puede generar una
frase vocal breve a partir de MIDI y letra, con una sincronización aceptable.

Orden recomendado:

1. Evaluar OpenUtau y un modelo compatible de síntesis vocal.
2. Comparar calidad y controles con ACE Studio.
3. Probar español e inglés con la misma melodía.
4. Medir afinación, timing, naturalidad, recursos y esfuerzo de edición.
5. Verificar licencias de todos los componentes.
6. Diseñar un adaptador de renderizado offline para Cantus.
7. Solo entonces decidir si conviene entrenar o adaptar un modelo propio.

## 11. Recursos iniciales

- ACE Studio: https://docs.acestudio.ai/
- OpenUtau: https://github.com/openutau/OpenUtau
- DiffSinger: https://github.com/MoonInTheRiver/DiffSinger

Estos recursos son puntos de partida para la investigación. Las capacidades, versiones,
idiomas admitidos y condiciones de licencia deben confirmarse en la documentación vigente
antes de tomar decisiones de implementación.
