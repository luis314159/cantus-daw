// Cantus icon family — set 6 additions A: the picker's groups for icons-a's glyphs, their
// names in English and Spanish and the words search also finds (spec-tracks-add-a.md lists
// them), and the variants drawn for the hard ones with the one chosen.
(() => {
  const N = window.N;
  // The chosen variants take the plain ids.
  const CHOSEN = { brushes: 'brushesA', edrums: 'edrumsA', castanets: 'castanetsB', marimba: 'marimbaA', cabasa: 'cabasaB', darbuka: 'darbukaA', quijada: 'quijadaA', steelpan: 'steelpanA', gamelanGong: 'gamelanGongB', crotales: 'crotalesA' };
  Object.entries(CHOSEN).forEach(([id, v]) => { N[id] = N[v]; });
  window.TRACK_VARIANTS_A = [
    ['Brushes: one, or a crossed pair', ['brushesA', 'brushesB'], 'brushesA'],
    ['Electronic drums: hexagonal pads, or round pads and the module', ['edrumsA', 'edrumsB'], 'edrumsA'],
    ['Castanets: the shells open on their hinge, or the pair face on', ['castanetsA', 'castanetsB'], 'castanetsB'],
    ['Marimba: with its mallets, or the bare instrument', ['marimbaA', 'marimbaB'], 'marimbaA'],
    ['Jawbone: from the side, or the arch of teeth from above', ['quijadaA', 'quijadaB'], 'quijadaA'],
    ['Cabasa: a grid of beads, or the chains round the core', ['cabasaA', 'cabasaB'], 'cabasaB'],
    ['Darbuka: upright, or lying as it is held (apart from the djembe)', ['darbukaA', 'darbukaB'], 'darbukaA'],
    ['Steel pan: the drum with its stick, or the notes seen from above', ['steelpanA', 'steelpanB'], 'steelpanA'],
    ['Gamelan gong: the hung bossed gong, or a bonang row of kettles', ['gamelanGongA', 'gamelanGongB'], 'gamelanGongB'],
    ['Crotales: discs in two rows on their board, or two thick discs on a post', ['crotalesA', 'crotalesB'], 'crotalesA'],
  ];
  window.TRACK_IDS_A = [
    ['Cymbals and kit pieces', ['crash', 'ride', 'splash', 'china', 'hihatOpen', 'hihatClosed', 'cymbalStack', 'rackTom', 'floorTom', 'sideStick', 'sticks', 'brushes', 'edrums', 'kickPedal']],
    ['Latin & Mexican percussion', ['bongos', 'timbales', 'guiro', 'claves', 'quijada', 'cabasa', 'shekere', 'agogo', 'cuica', 'surdo', 'pandeiro', 'woodblock', 'castanets', 'vibraslap', 'marimba']],
    ['World percussion', ['djembe', 'darbuka', 'tabla', 'frameDrum', 'taiko', 'handpan', 'steelpan', 'kalimba', 'rainstick', 'windChimes', 'tubularBells', 'gamelanGong', 'crotales']],
  ];
  // id: [English, Spanish, other search words]
  window.TRACK_WORDS_A = {
    crash: ['crash', 'platillo crash', 'crash cymbal, choque'],
    ride: ['ride', 'platillo ride', 'ride cymbal, campana, bell'],
    splash: ['splash', 'platillo splash', 'splash cymbal'],
    china: ['china', 'platillo china', 'china cymbal, trash'],
    hihatOpen: ['hi-hat open', 'charles abierto', 'open hat, oh, contratiempo abierto'],
    hihatClosed: ['hi-hat closed', 'charles cerrado', 'closed hat, ch, contratiempo cerrado'],
    cymbalStack: ['stacked cymbals', 'platillos apilados', 'stack, efecto'],
    rackTom: ['rack tom', 'tom aéreo', 'high tom, tom alto, tom de rack'],
    floorTom: ['floor tom', 'tom de piso', 'low tom, timbal base, goliat'],
    sideStick: ['side-stick', 'aro', 'rim, rimshot, cross-stick, golpe de aro'],
    sticks: ['drumsticks', 'baquetas', 'sticks, palillos, palos'],
    brushes: ['brushes', 'escobillas', 'brochas, cepillos, jazz'],
    edrums: ['electronic drums', 'batería electrónica', 'e-drums, pads, electronic kit'],
    kickPedal: ['kick pedal', 'pedal de bombo', 'bass drum pedal, beater, mazo'],
    bongos: ['bongos', 'bongós', 'bongo, bongó'],
    timbales: ['timbales', 'timbales latinos', 'pailas, timbaletas, paila'],
    guiro: ['güiro', 'güiro', 'guiro, guayo, rascador, scraper'],
    claves: ['claves', 'claves', 'clave, palitos'],
    quijada: ['jawbone', 'quijada', 'quijada de burro, charrasca'],
    cabasa: ['cabasa', 'cabasa', 'afoxé, afuche, afoxe'],
    shekere: ['shekere', 'chekeré', 'shekeré, agbe, güiro de cuentas'],
    agogo: ['agogô', 'agogó', 'agogo, campana doble'],
    cuica: ['cuíca', 'cuíca', 'cuica, puerca, zambomba'],
    surdo: ['surdo', 'surdo', 'bombo brasileño, samba'],
    pandeiro: ['pandeiro', 'pandeiro', 'pandero brasileño'],
    woodblock: ['woodblock', 'caja china', 'wood block, bloque de madera, temple block'],
    castanets: ['castanets', 'castañuelas', 'palillos'],
    vibraslap: ['vibraslap', 'vibraslap', 'quijada (efecto), vibraeslap'],
    marimba: ['marimba', 'marimba', 'marimba mexicana, marimba de Chiapas, tuned'],
    djembe: ['djembe', 'yembé', 'djembé, jembe'],
    darbuka: ['darbuka', 'darbuka', 'doumbek, derbake, tabla árabe, goblet drum'],
    tabla: ['tabla', 'tabla', 'tablas, bayan, dayan'],
    frameDrum: ['frame drum', 'pandero de marco', 'bodhrán, bendir, daf, tar'],
    taiko: ['taiko', 'taiko', 'tambor japonés, wadaiko'],
    handpan: ['handpan', 'handpan', 'hang, pantam, tuned'],
    steelpan: ['steel pan', 'tambor metálico', 'steel drum, steelpan, pan, tuned'],
    kalimba: ['kalimba', 'kalimba', 'mbira, sanza, piano de pulgar, thumb piano, tuned'],
    rainstick: ['rainstick', 'palo de lluvia', 'palo de agua, rain stick'],
    windChimes: ['wind chimes', 'carillón de viento', 'mark tree, chimes, campanas de viento'],
    tubularBells: ['tubular bells', 'campanas tubulares', 'chimes, orchestral bells, tuned'],
    gamelanGong: ['gamelan gong', 'gong de gamelán', 'gamelan, bonang, gong ageng, kempul, tuned'],
    crotales: ['crotales', 'crótalos', 'antique cymbals, platillos antiguos, tuned'],
  };
})();
