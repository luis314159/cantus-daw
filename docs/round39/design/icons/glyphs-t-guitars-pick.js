// Cantus icon family — the guitar family, the chosen variants (round 39, icons-guitars), picked
// by looking at 12, 14, 16 and 24 px at 1:1 and 4×, every type beside its neighbours
// (sheet-guitars.html). guitars-study.md says why each was chosen.
(() => {
  const N = window.N;
  window.GUITAR_PICK = [
    // id, variants, chosen, why
    ['eguitar', ['eguitarA', 'eguitarB', 'eguitarC'], 'eguitarB', 'the double cutaway at 45° on a neck long enough to make the emoji-like guitar sign, its body as large as the key allows; the cable (C) was a smudge at 16 px'],
    ['aguitar', ['aguitarA', 'aguitarB', 'aguitarC'], 'aguitarB', 'the dreadnought at 45° with its hole and bridge, the same pose and size as the electric'],
    ['baritone', ['baritoneA', 'baritoneB', 'baritoneC'], 'baritoneA', 'a smaller offset body on a far longer neck at 45°: the long scale is the name; upright (C) it read as a bass'],
    ['eStype', ['eStypeA', 'eStypeB', 'eStypeC'], 'eStypeC', 'two rounded horns, the bass horn clearly the taller, a contoured waist, three pickups'],
    ['eTtype', ['eTtypeA', 'eTtypeB', 'eTtypeC'], 'eTtypeC', 'the slab: a flat ledge and a square shoulder on the bass side, straight sides, one blunt horn'],
    ['eSingleCut', ['eSingleCutA', 'eSingleCutB', 'eSingleCutC'], 'eSingleCutC', 'a round dome of a shoulder over a deep waist, one sharp horn pointing up, two humbuckers'],
    ['eDoubleHorn', ['eDoubleHornA', 'eDoubleHornB', 'eDoubleHornC'], 'eDoubleHornC', 'two sharp horns spreading like a lyre, symmetric, a slim tulip of a body'],
    ['eSuperstrat', ['eSuperstratA', 'eSuperstratB', 'eSuperstratC'], 'eSuperstratC', 'needle horns pointing straight up, the bass horn far the longer, a pointed head'],
    ['eSemiHollow', ['eSemiHollowA', 'eSemiHollowB', 'eSemiHollowC'], 'eSemiHollowA', 'the widest electric, round symmetric ears, the f-holes cut at every size'],
    ['eOffset', ['eOffsetA', 'eOffsetB', 'eOffsetC'], 'eOffsetC', 'skewed, not turned: a broad bass horn, the waist high on the bass side and low on the treble side, the lowest point right of the axis'],
    ['eVshape', ['eVshapeA', 'eVshapeB', 'eVshapeC'], 'eVshapeA', 'two wings and a notch'],
    ['ePointed', ['ePointedA', 'ePointedB', 'ePointedC'], 'ePointedC', 'few, bold points: the bass horn up and out, a treble point, a long tail to the lower right'],
    ['archtop', ['archtopA', 'archtopB', 'archtopC'], 'archtopA', 'the big hollow body with one round cutaway, f-holes, the tailpiece'],
    ['aDread', ['aDreadA', 'aDreadB', 'aDreadC'], 'aDreadA', 'square shoulders, a shallow waist'],
    ['aConcert', ['aConcertA', 'aConcertB', 'aConcertC'], 'aConcertA', 'a small round upper bout over a deep waist'],
    ['aCutaway', ['aCutawayA', 'aCutawayB', 'aCutawayC'], 'aCutawayA', 'the dreadnought with its treble shoulder scooped to the waist'],
    ['aJumbo', ['aJumboA', 'aJumboB', 'aJumboC'], 'aJumboA', 'the widest, roundest lower bout under a round upper bout, pinched between'],
    ['aParlor', ['aParlorA', 'aParlorB', 'aParlorC'], 'aParlorA', 'small and narrow, low on a long neck'],
    ['classical', ['classicalA', 'classicalB', 'classicalC'], 'classicalB', 'the slotted head (cut), a wider neck, the rosette ring from 24 px'],
    ['guitar12', ['guitar12A', 'guitar12B', 'guitar12C'], 'guitar12A', 'a long head with a comb of tuners down both sides'],
    ['aTravel', ['aTravelA', 'aTravelB', 'aTravelC'], 'aTravelA', 'the whole guitar small in its key, its head well under the top'],
    ['resonator', ['resonatorA', 'resonatorB', 'resonatorC'], 'resonatorA', 'the round cover plate in the second ink, ringed by a cut'],
    ['bassguitar', ['bassguitarA', 'bassguitarB', 'bassguitarC'], 'bassguitarA', 'upright, a small offset body low on a long neck, four tuners in line, the split pickup'],
    ['fretless', ['fretlessA', 'fretlessB', 'fretlessC'], 'fretlessA', 'the bass with its fingerboard in the second ink, smooth'],
    ['acousticbass', ['acousticbassA', 'acousticbassB', 'acousticbassC'], 'acousticbassA', 'an acoustic body with its hole on a bass neck, two tuners a side'],
    ['requinto', ['requintoA', 'requintoB', 'requintoC'], 'requintoA', 'the slender body and the espiga standing beside it'],
    ['vihuela', ['vihuelaA', 'vihuelaB', 'vihuelaC'], 'vihuelaA', 'a small round body, its vaulted back beside it in the second ink'],
    ['guitarron', ['guitarronA', 'guitarronB', 'guitarronC'], 'guitarronA', 'the huge body on a short neck, its bowl back beside it'],
    ['bajosexto', ['bajosextoA', 'bajosextoB', 'bajosextoC'], 'bajosextoA', 'a tall pear met high by the neck, a comb of twelve tuners'],
    ['jarana', ['jaranaA', 'jaranaB', 'jaranaC'], 'jaranaA', 'slim, pinched, its hole low, a wide head'],
    ['charango', ['charangoA', 'charangoB', 'charangoC'], 'charangoA', 'the banded shell'],
    ['cuatro', ['cuatroA', 'cuatroB', 'cuatroC'], 'cuatroA', 'the corners at the waist'],
    ['tres', ['tresA', 'tresB', 'tresC'], 'tresC', 'three courses down the body'],
  ];
  window.GUITAR_PICK.forEach(([id, , pick]) => { N[id] = N[pick]; });
  window.GUITAR_FAMS = [
    ['Guitars & basses', ['eguitar', 'aguitar', 'bassguitar', 'archtop', 'resonator', 'baritone', 'fretless', 'acousticbass']],
    ['Electric shapes', ['eStype', 'eTtype', 'eSingleCut', 'eDoubleHorn', 'eSuperstrat', 'eSemiHollow', 'eOffset', 'eVshape', 'ePointed']],
    ['Acoustic shapes', ['aDread', 'aConcert', 'aCutaway', 'aJumbo', 'aParlor', 'classical', 'guitar12', 'aTravel']],
    ['Mexican & Latin strings', ['requinto', 'vihuela', 'guitarron', 'bajosexto', 'jarana', 'charango', 'cuatro', 'tres']],
  ];
})();
