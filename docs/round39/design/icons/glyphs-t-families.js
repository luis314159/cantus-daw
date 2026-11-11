// Cantus icon family — set 6, the whole track-icon library (round 39, icons-final): the
// picker's families in the owner's order (set6-additions.md, "The picker"), and the words
// search finds. Loaded after every glyphs-t-*.js file and glyphs-t-final.js.
//
// A family is [title, ids, subs?]; subs are [title, ids] shown under the family's own row.
// An id listed in two families is one drawing (bass, metronomeTrack, fire, bell); the
// library counts it once.
(() => {
  window.TRACK_FAMILIES = [
    ['Drums', ['kick', 'snare', 'tom', 'rackTom', 'floorTom', 'drumkit', 'edrums', 'sideStick', 'sticks', 'brushes', 'kickPedal',
      'clap', 'shaker', 'tambourine', 'triangleInst']],
    ['Cymbals', ['hihat', 'cymbal', 'hihatClosed', 'hihatOpen', 'crash', 'ride', 'splash', 'china', 'cymbalStack']],
    ['Latin & Mexican percussion', ['congas', 'bongos', 'timbales', 'cajon', 'cowbell', 'guiro', 'claves', 'quijada', 'cabasa', 'shekere',
      'agogo', 'cuica', 'surdo', 'pandeiro', 'woodblock', 'castanets', 'vibraslap', 'marimba']],
    ['World percussion', ['djembe', 'darbuka', 'tabla', 'frameDrum', 'taiko', 'handpan', 'steelpan', 'kalimba', 'rainstick', 'windChimes',
      'gong', 'gamelanGong']],
    ['Tuned percussion', ['xylophone', 'vibraphone', 'glockenspiel', 'tubularBells', 'crotales', 'bells', 'timpani', 'musicbox']],
    ['Guitars & basses', ['eguitar', 'aguitar', 'bassguitar', 'archtop', 'resonator', 'lapsteel', 'pedalsteel', 'baritone', 'fretless',
      'acousticbass', 'guitaramp', 'bassamp', 'pedal'], [
      ['Electric shapes', ['eStype', 'eTtype', 'eSingleCut', 'eDoubleHorn', 'eSuperstrat', 'eSemiHollow', 'eOffset', 'eVshape', 'ePointed']],
      ['Acoustic shapes', ['aDread', 'aConcert', 'aCutaway', 'aJumbo', 'aParlor', 'classical', 'guitar12', 'aTravel']],
    ]],
    ['Mexican & Latin strings', ['requinto', 'vihuela', 'guitarron', 'bajosexto', 'jarana', 'charango', 'cuatro', 'tres']],
    ['Strings', ['violin', 'viola', 'cello', 'doubleBass', 'stringsection', 'harp', 'banjo', 'mandolin', 'ukulele', 'sitar', 'erhu', 'koto',
      'guzheng', 'shamisen', 'pipa', 'oud', 'lute', 'bouzouki', 'balalaika', 'hdulcimer', 'mdulcimer', 'zither', 'hurdygurdy']],
    ['Winds & brass', ['sax', 'sopranosax', 'trumpet', 'cornet', 'flugelhorn', 'trombone', 'horn', 'euphonium', 'tuba', 'sousaphone',
      'hornsection', 'flute', 'piccolo', 'clarinet', 'bassclarinet', 'oboe', 'englishhorn', 'bassoon', 'woodwindsection', 'recorder',
      'tinwhistle', 'panflute', 'shakuhachi', 'ocarina', 'didgeridoo', 'bagpipes', 'harmonica', 'melodica', 'kazoo', 'accordion', 'bandoneon']],
    ['Keys', ['piano', 'upright', 'harpsichord', 'celesta', 'toypiano', 'organ', 'tonewheel', 'epiano', 'tinepiano', 'reedpiano', 'clavinet',
      'mellotron', 'synthkeys', 'midikeyboard', 'keytar', 'keys']],
    ['Electronic & production', ['synth', 'pad', 'lead', 'pluck', 'arp', 'bass', 'sub', 'boom808', 'acid', 'drummachine', 'sampler', 'padgrid',
      'groovebox', 'stepseq', 'modular', 'theremin', 'vocoder', 'talkbox', 'turntable', 'djcontroller', 'cdj', 'laptop', 'midi', 'loop',
      'oneshot', 'cassette', 'radio', 'chiptune', 'glitch', 'waveSine', 'waveSaw', 'waveSquare', 'waveTri']],
    ['Voice', ['mic', 'leadVocal', 'voice', 'choir', 'backingVocals', 'harmony', 'rap', 'vocalChop', 'adlib', 'whisper', 'beatbox', 'whistle',
      'podcast', 'talkback']],
    ['Notation', ['trebleClef', 'bass', 'cClef', 'percClef', 'quarterNote', 'eighthNotes', 'wholeNote', 'rest', 'sharp', 'flat', 'fermata',
      'tuningFork', 'scoreSheet', 'metronomeTrack']],
    ['Sound design & Foley', ['riser', 'downlifter', 'sweep', 'whoosh', 'impact', 'subDrop', 'noise', 'reverse', 'stutter', 'tapeStop',
      'vinyl', 'scratch', 'footsteps', 'ambience', 'rain', 'wind', 'water', 'fire', 'birds', 'crowd', 'siren', 'explosion', 'phone',
      'fieldRec', 'bell', 'video']],
    ['Mix & routing', ['bus', 'groupFolder', 'aux', 'reverb', 'delay', 'sidechain', 'compressor', 'eq', 'limiter', 'saturator', 'filterFx',
      'chorus', 'phaser', 'master', 'mastering', 'stems', 'reference', 'metronomeTrack', 'cue', 'headphones', 'monitor', 'monoMix',
      'stereoMix', 'surround']],
    ['Marks & shapes', ['star', 'heart', 'flag', 'circle', 'square', 'triangle', 'diamond', 'hexagon', 'bolt', 'fire', 'sun', 'moon', 'leaf',
      'cross', 'checkMark', 'musicNote', 'bookmark', 'pin', 'eye', 'lock', 'smile', 'bell']],
    ['Letters & numbers', ['letterA', 'letterB', 'letterC', 'letterD', 'letterE', 'letterF', 'letterG', 'letterH',
      'num1', 'num2', 'num3', 'num4', 'num5', 'num6', 'num7', 'num8', 'num9']],
  ];
  // Every id of a family, its subs' after its own.
  window.familyIds = f => f[1].concat(...(f[2] || []).map(s => s[1]));
  window.TRACK_LIBRARY = [...new Set(window.TRACK_FAMILIES.flatMap(window.familyIds))];

  // The words: set 6's first 82, then the three additions; the final renames win.
  const W = window.TRACK_WORDS = Object.assign({}, window.TRACK_WORDS, window.TRACK_WORDS_A || {}, window.ADDC_WORDS || {});
  const set = (id, en, es, more) => { const o = W[id] || ['', '', '']; W[id] = [en ?? o[0], es ?? o[1], more ?? o[2]]; };
  // Labels never carry a maker's model name; those stay search words.
  set('eSuperstrat', 'pointed double cut', 'doble corte afilado', 'superstrat, shred, metal, pointed horns, cuernos puntiagudos');
  set('eStype', 'double cutaway', 'doble cutaway', 'strat, s-type, tipo S, double cutaway, doble corte');
  set('eTtype', 'single cutaway slab', 'cutaway simple plana', 'tele, t-type, tipo T, single cutaway, un corte');
  set('eSingleCut', 'carved single cut', 'tallada, un corte', 'les paul, lp, single cut, tapa tallada');
  set('eDoubleHorn', 'twin-horn solid body', 'sólida de dos cuernos', 'sg, horns, cuernos, devil horns');
  set('eSemiHollow', 'semi-hollow', 'semihueca', '335, es-335, f-holes, efes, semiacústica');
  set('eOffset', 'offset body', 'cuerpo offset', 'jazzmaster, jaguar, offset, asimétrica, surf, indie');
  set('eVshape', 'V body', 'cuerpo en V', 'flying v, v, metal');
  set('ePointed', 'pointed body', 'cuerpo puntiagudo', 'explorer, warlock, pointy, metal, picos');
  set('aCutaway', 'cutaway dreadnought', 'dreadnought con corte', 'cutaway, electroacústica, dreadnought con cutaway');
  set('tonewheel', 'tonewheel organ', 'órgano de ruedas', 'drawbars, hammond, b3, órgano de ruedas fónicas, órgano eléctrico, jazz organ');
  set('tinepiano', 'tine electric piano', 'piano de púas', 'rhodes, suitcase, ep, tines, e-piano, piano eléctrico de púas');
  set('reedpiano', 'reed electric piano', 'piano de lengüetas', 'wurlitzer, wurli, ep, reeds, e-piano, piano eléctrico de lengüetas');
  set('hihat', 'hi-hat', 'charles', 'hat, hh, contratiempo, hi hat');
  set('cymbal', 'cymbal', 'platillo', 'cymbals, platillos, címbalo, plato');
  set('triangleInst', 'triangle', 'triángulo', 'instrument, instrumento, ting');
  set('congas', 'congas', 'congas', 'conga, tumbadora, quinto, tumba');
  set('timpani', 'timpani', 'timbales sinfónicos', 'kettle drum, tímpanos, timbal de orquesta');
  set('bass', 'bass (synth)', 'bajo (sinte)', 'bass clef, clave de fa, fa, F clef, clef, clave, synth bass');
  // An icon in two families may be named for each.
  window.TRACK_LABEL_IN = { 'Notation:bass': ['bass clef', 'clave de fa'], 'Marks & shapes:fire': ['flame', 'llama'], 'Marks & shapes:bell': ['bell', 'campana'] };
  set('mic', 'vocal mic', 'micrófono', 'vocals, voz, mic, microphone');
  // Words that now name an icon of their own leave the older icon's aliases.
  set('violin', null, null, 'fiddle, violino');
  set('accordion', null, null, 'acordeon, squeezebox');
  set('choir', null, null, 'chorus, ensemble, voces, coral');
  set('riser', null, null, 'fx, uplifter, build-up, subida');
  set('sampler', null, null, 'sample, muestreador, sampling');
  set('drummachine', null, null, 'rhythm machine, ritmos, beat machine');
  set('xylophone', 'xylophone', 'xilófono', 'xylo, mallets, láminas');
})();
