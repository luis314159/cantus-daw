"""The words on each character's sheet (v2: the cast)."""

INFO = {
    "compas": {
        "letter": "A",
        "title": "Compás — the pictogram",
        "idea": ("A sign figure come alive. One ink (the theme's ink 1), solid pieces of one weight "
                 "with round ends — a disc for a head, a capsule torso, bars for limbs — each cut from "
                 "the piece behind it by a hairline of the room. Gold only on the headphones and on the "
                 "beat ring at its feet. It dances pose to pose: it snaps into a key on the beat and "
                 "holds it (pop and lock), with a pictogram's motion marks on the snap. No face: a mood "
                 "is a posture, or negative space cut into the disc."),
        "why": ["The calmest of the cast and the most Slate: one ink, one accent, like the icon family.",
                "Reads at 48 px and over any clip; the cut keeps it legible on a busy window.",
                "Fun from timing, not detail: the snaps land on the beat, the holds let the eye read the pose.",
                "Cheapest to draw: about 20 fills a frame (the cast draws 52–113)."],
        "risk": "It can read as a sign, not a character: its charm is in the motion, so judge the GIFs, not the stills.",
    },
    "tono": {
        "letter": "B",
        "title": "Tono — the knob (v2)",
        "idea": ("Slate's parts with an attitude. A knob for a head — its rim, a lit lip, the scale of "
                 "dots lit gold by the level, the gold pointer a fin that swings late — on a lean robot "
                 "body: a key with the logo's five bars in a well, straight limbs with joints, fists, "
                 "high-top fader-cap sneakers. Its eyes are gold segments in an LED visor (no mouth: the "
                 "five bars are its voice); a vermilion patch cable is slung round its neck. Signature: "
                 "Turn it up — it reaches up and turns its own knob, three clicks, the dots filling."),
        "why": ["The most Cantus of the cast: made of the window it lives in, and it wears the theme.",
                "Cool, not cute: a smaller head, a visor, a lean back, a hard nod with fist hits.",
                "Its signature is the app's own gesture: turning a knob up to the top.",
                "Close to today's rig: the family's construction with new parts."],
        "risk": "The visor recalls a helmet; keep the knob's rim, dots and pointer readable so it stays a knob.",
    },
    "sincopa": {
        "letter": "C",
        "title": "Síncopa — the figure in values (v2)",
        "idea": ("An editorial figure with swagger. No ink line: every piece two flat values. The logo's "
                 "colours as streetwear: an open oversized vermilion bomber over a cream tee, lapis cargo "
                 "trousers with a strap, high-tops, gold headphones round the neck, long braids in a high "
                 "ponytail with gold beads that swing late, a gold hoop, half-lidded eyes and a smirk. "
                 "It hits and overshoots, a shoulder pops on the off-beat, chin up. Signature: Shades — "
                 "the sunglasses go on, the hand flicks away, it freezes leaning back."),
        "why": ["The closest to a real dancer, now with attitude: the 'cool' Fuga reached for, calm and editorial.",
                "Secondary motion sells it: braids, beads and the strap follow late.",
                "On brand: lapis, gold and vermilion are its clothes.",
                "A signature anyone reads at a glance: sunglasses on, on the drop."],
        "risk": "The most detailed figure; its face is lost under 200 px, and it needs IK for every limb.",
    },
    "bajo": {
        "letter": "D",
        "title": "Bajo — the speaker",
        "idea": ("A studio monitor with a heavy, calm groove. The cabinet is its body and head: two tweeters "
                 "are its eyes (gold domes for pupils, heavy lids of the baffle for moods), the woofer "
                 "is its chest and pumps with the music, the bass port its belt; short thick legs on "
                 "isolation pads, big mitts. Graphite in both themes, held at 3:1 on Night by the knob's "
                 "rim. Signature: Drop the bass — it winds up, then the woofer blasts gold rings, the "
                 "cabinet jolts and shakes."),
        "why": ["A silhouette nobody else in the cast has: a tall box, weight, slowness.",
                "Unimpressed, half-lidded eyes: cool by being calm.",
                "A visualiser in its chest (the woofer and its dust cap pump with the level).",
                "Simple to draw: rounded rects and circles, about 35 fills."],
        "risk": "A box with legs can read as a prop; its charm is in the weight of the motion and the blast.",
    },
    "onda": {
        "letter": "E",
        "title": "Onda — the wave",
        "idea": ("A living ribbon, the most abstract of the family. It rises from a curl on the floor as a "
                 "travelling wave to a round head and twists as it moves, so its front (lapis) and its "
                 "back (gold) take turns — a waveform you could pick up. Faceless like Compás: its eyes "
                 "are negative space cut into the head. It dances as a wave: the amplitude pulses on the "
                 "beat; signature: Square drop — it springs up and the sine becomes a square wave, then a "
                 "saw, then melts back."),
        "why": ["Pure motion design: the hypnotic one, beautiful on a stream.",
                "Its signature is sound itself: sine, square, saw — a synth's waveforms.",
                "Compás's sibling in spirit (cut eyes, two colours), the opposite silhouette (all curve).",
                "Cheap: two ribbon fills, two shade strips, the head — about 10 fills."],
        "risk": "Without limbs it cannot do every move of the sixteen literally; its moves are re-read as waves.",
    },
    "voz": {
        "letter": "F",
        "title": "Voz — the microphone",
        "idea": ("A crooner, smooth and showy. Its head is a vintage microphone's capsule — fine grille "
                 "bars, a gold ring — under a pork-pie hat with a gold band. It wears the evening: a "
                 "dinner jacket white at night and black on paper (it inverts with the theme), a gold bow "
                 "tie, a vermilion pocket square, spectator shoes, white gloves. A hand to the heart, then "
                 "out to the crowd; a finger-snapping slide; signature: Hat tip — a bow, then both arms "
                 "flung out, ta-da."),
        "why": ["Showman cool: the elegant one of the cast, a different energy from the street and the robot.",
                "The theme inversion is its joke and its legibility: always the strongest value in the room.",
                "A head of the studio's oldest object, a capsule mic, read at once.",
                "The humanoid skeleton Síncopa needs, a simpler face."],
        "risk": "Object-head characters are a known illustration trope; its hat, ring and suit must keep it ours.",
    },
}

INFO.update({
    "sincopa_s1": {
        "letter": "C · S1", "title": "Síncopa, graphic",
        "idea": ("A bold, simplified human in strong geometric shapes: Compás's clarity with clothes, hair "
                 "and a face. One flat value per piece, no shading; limbs are bars of one width; the head a "
                 "disc; a sharp vermilion A-line bob is the silhouette you remember; the face is dots and a "
                 "line. It wears the theme as Compás does: a lapis jacket, wide trousers in the theme's ink "
                 "(white at night, near-black on paper), white sneakers on gold soles, gold hoops. Graphic "
                 "hits on the half beat. Signature: Spin and point — a paper-cut spin, its back shown in the "
                 "turn, landing on the beat into a disco point."),
        "why": ["The closest to Compás: the same clarity, so the cast reads as one family.",
                "The bob and the suit-like trousers make a silhouette that reads at 48 px.",
                "No skin rendering, no detail to go wrong: it stays crisp at any size.",
                "Cheap: about 25 fills; the spin is a scale of the drawing."],
        "risk": "Flat human figures are common in product illustration; the bob, the colours and the snaps must carry it.",
    },
    "sincopa_s2": {
        "letter": "C · S2", "title": "Síncopa, the sidekick",
        "idea": ("Feature-animation proportions and a face that acts: a big head, big hands and feet, long thin "
                 "legs; huge eyes with pupils and a highlight, thick brows, a wide mouth that grins with "
                 "teeth and shouts; a curly puff with a gold headband that bounces after the head. An "
                 "oversized vermilion tee, lapis shorts, striped tube socks, chunky sneakers with gold laces. "
                 "Big anticipation, squash and stretch, fists pumped over the head. Signature: Hype jump — a "
                 "deep crouch, a stretched launch, a star with a shout, a squashed landing with dust."),
        "why": ["The most expressive: a face you read from across the room, emotions at a glance.",
                "Energy: it sells the music's excitement like an animated sidekick.",
                "Big shapes stay readable small: the head and the hands are the silhouette.",
                "Two values per piece, like the family; no lines."],
        "risk": "Fun can tip into childish; it is the opposite answer to 'cool' — it is here to be compared.",
    },
    "sincopa_s3": {
        "letter": "C", "title": "Síncopa — noir (chosen)",
        "idea": ("A human drawn as a silhouette: one dark ink for the whole figure, a gold rim light down its "
                 "lit side and a cool rim down the other (that draws it on Night's dark room; on Paper it is "
                 "a sharp silhouette). A hood with gold headphones over it, a long open coat whose tails "
                 "swing late, slim trousers, sneakers with gold soles that flash when it steps. The face is "
                 "two gold glints in the hood's dark. Liquid motion, behind the beat. Signature: Arm wave — a "
                 "wave runs from one hand through the shoulders out of the other; then it looks, eyes lit."),
        "why": ["Mysterious and stylish: the most 'cool' answer, nothing cute about it.",
                "Slate's own idea — light as value — on a body: the rim light is the drawing.",
                "One ink and one gold: the calmest palette of the three, like Compás.",
                "Cheap: each piece is three fills (two rims, the ink), about 40 in all."],
        "risk": "A faceless figure keeps people at a distance; its moods are only glints and posture.",
    },
})

PALETTES = {
    "compas": [("Figure — ink 1", "#E3E8ED", "#17202A"), ("Cut — the room", "#1D2329", "#F4F6F8"),
               ("Gold — headphones, ring", "#E2B85A", "#9A6E1C"), ("Motion marks — ink 3", "#909CA8", "#556170")],
    "tono": [("Knob", "#2F373F", "#FBFCFD"), ("Rim — holds 3:1", "#76818D", "#7A8692"),
             ("Visor slot", "#0D1014", "#1D2329"), ("LED eyes", "#F2C766", "#F2C766"),
             ("Limbs", "#7D8996", "#6A7682"), ("Patch cable", "#F0784A", "#D2541A"),
             ("Gold — pointer, dots", "#E2B85A", "#9A6E1C"), ("Key body", "#2B3239", "#E4E9EE")],
    "sincopa": [("Bomber", "#E0693B", "#E0693B"), ("Bomber shade", "#B24E29", "#B24E29"),
                ("Tee", "#EDE6DA", "#EDE6DA"), ("Cargo (lapis)", "#4664C0", "#4664C0"),
                ("Skin", "#C08463", "#C08463"), ("Braids (lit)", "#7A5E52", "#7A5E52"),
                ("High-tops", "#F3F5F7", "#F3F5F7"), ("Gold — beads, hoop, phones", "#E2B85A", "#E2B85A")],
    "bajo": [("Cabinet", "#262D34", "#262D34"), ("Rim — holds 3:1", "#76818D", "#76818D"),
             ("Cone", "#4C5661", "#4C5661"), ("Dust cap, domes", "#E2B85A", "#E2B85A"),
             ("Limbs", "#6A7682", "#6A7682"), ("Mitts", "#B4BDC6", "#B4BDC6"),
             ("Blast rings — gold", "#E2B85A", "#9A6E1C"), ("Port, surround", "#0D1014", "#0D1014")],
    "onda": [("Front (lapis)", "#7D97E6", "#2E4794"), ("Front shade", "#5C78CC", "#24397A"),
             ("Back (gold)", "#E2B85A", "#B8882C"), ("Back shade", "#B38A35", "#8F6A1E"),
             ("Cut eyes — the room", "#1D2329", "#F4F6F8")],
    "voz": [("Jacket", "#E9E4DA", "#232A31"), ("Jacket shade", "#BDB4A4", "#14191E"),
            ("Grille", "#C3CBD3", "#6A7682"), ("Trousers", "#59636E", "#2B3239"),
            ("Hat", "#2B3239", "#2B3239"), ("Gold — ring, band, tie", "#E2B85A", "#C99A3A"),
            ("Pocket square", "#F0784A", "#D2541A"), ("Gloves", "#F3F5F7", "#F3F5F7")],
}

PALETTES.update({
    "sincopa_s1": [("Jacket (lapis)", "#6A86DA", "#2E4794"), ("Trousers — ink 1", "#E3E8ED", "#17202A"),
                   ("Bob (vermilion)", "#F0784A", "#D2541A"), ("Skin", "#B97A57", "#B97A57"),
                   ("Sneakers", "#F3F5F7", "#F3F5F7"), ("Gold — soles, hoops", "#E2B85A", "#B8882C")],
    "sincopa_s2": [("Tee", "#E0693B", "#E0693B"), ("Shorts (lapis)", "#4664C0", "#4664C0"),
                   ("Skin", "#8D5A3B", "#8D5A3B"), ("Hair (lit)", "#5C4238", "#5C4238"),
                   ("Eyes", "#FFFFFF", "#FFFFFF"), ("Gold — headband, laces", "#E2B85A", "#E2B85A")],
    "sincopa_s3": [("Ink — the figure", "#0E1216", "#0E1216"), ("Gold rim light", "#E2B85A", "#C99A3A"),
                   ("Cool rim", "#8A96A2", "#7A8692"), ("Glints — eyes", "#F2C766", "#F2C766")],
})

CONSTRUCTION = {
    "compas": ["Head: a disc, r 0.092 (units: the figure stands 1 tall).",
               "Torso: a capsule 0.162 → 0.120 wide; legs 0.078, arms 0.066, round ends.",
               "Cut: 0.017 of the room's colour round each piece in front (3 px at 200 px).",
               "Legs and torso are one silhouette (no cut between them); arms cut where they cross.",
               "Headphones: a gold band 0.017 and two cups 0.040 × 0.066, cut from the disc.",
               "Beat ring: a gold ellipse at its feet, opaque, shrinking after each beat."],
    "tono": ["Head: Slate's knob, r 0.165, rim 0.011, a dark cut outside; 11 dots lit by the level.",
             "Visor: a slot 1.42 × 0.40 of the radius; gold segments shaped by the mood.",
             "Pointer: a gold fin through the rim, swinging late; Turn it up turns it 135°.",
             "Body: a key 0.235 wide with rim, lip and a dark well with five vermilion bars.",
             "Limbs: straight segments 0.046–0.048 with joint discs; fists r 0.034; high-tops.",
             "A vermilion cable round the neck, its gold plug swinging after the body."],
    "sincopa": ["About 8 heads tall; the head drawn ×1.08 for a readable face.",
                "Every piece two values: the shade, then the base moved toward the key light.",
                "Bomber: two halves over a cream tee; sleeves 0.086 → 0.066 with ribbed cuffs.",
                "Braids: a tapered tube with braid marks and gold beads, swinging late (the pointer's lag).",
                "Cargo: 0.106 → 0.088 → 0.100, a pocket flap, a strap that swings.",
                "Face: half-lidded eyes, angled brows, a smirk; Shades puts sunglasses on."],
    "bajo": ["Cabinet: 0.46 × 0.80, radius 0.06, rim, lip, baffle; rigid, turned about the pelvis.",
             "Tweeters: r 0.060 rings with gold domes; lids of the baffle make the moods.",
             "Woofer: surround r 0.168, cone (two values) pumping ×(1 + 0.07 level).",
             "Legs 0.072 on isolation pads 0.16 × 0.052; arms 0.060, mitts r 0.054.",
             "Drop the bass: three gold rings out from the woofer, their width shrinking.",
             "Squash and stretch on the cabinet, a shake after the blast."],
    "onda": ["A centreline of 60 samples, a wave travelling up it: sine → square (shape) → saw.",
             "Width 0.040 at the floor → 0.145 under the head; a round head at the top.",
             "Twist: strongest at the tail, none at the head (the face always shows).",
             "Front lapis / back gold by the twist's sign, each with a shade strip (two fills each).",
             "Eyes: negative space in the room's colour, as Compás's visor.",
             "A curl on the floor (a ribbon) where it stands."],
    "voz": ["Humanoid, about 7.5 heads; the head a capsule 0.150 × 0.215 with 15 grille bars.",
            "A gold ring under the capsule; the mic's body is its neck.",
            "Hat: brim 0.236 × 0.026, crown 0.150 × 0.060, a gold band; tipped about the brim.",
            "Jacket: two halves with lapels, inverted with the theme; gold bow tie, vermilion square.",
            "Spectator shoes (white and black), white gloves.",
            "Moods: eyes on the grille; Hat tip lifts and turns the hat 22°."],
}

CONSTRUCTION.update({
    "sincopa_s1": ["About 6 heads; head a disc r 0.092; limbs bars 0.090 (legs), 0.070 (arms).",
                   "One flat value per piece; pieces cut by 0.015 of the room's colour.",
                   "Bob: a dome r 0.108, sides to the jaw, longer at the front, a side-swept fringe.",
                   "Face: two dots, one line; sunglasses are one black bar with a gold glint.",
                   "Spin: the drawing's width through cos(turn); its back (no face) when negative.",
                   "Legs and jacket one silhouette; jacket's corners rounded by a 0.03 stroke."],
    "sincopa_s2": ["About 3.5 heads: head 0.30 tall, long thin legs (0.40), big hands and shoes.",
                   "Eyes: white 0.076 × 0.096, pupils r 0.021 with a highlight; thick brows.",
                   "Mouth: a smile line, or an open D with teeth and a tongue sized by the mouth.",
                   "Hair: an 18-bump puff, two values, bouncing with the accent; a gold headband.",
                   "Two values per piece (the rig's hard cel); no lines; dust on landing.",
                   "Squash up to 0.22 in the crouch, stretch 0.16 in the launch."],
    "sincopa_s3": ["About 8 heads; one ink #0E1216 for every piece.",
                   "Each piece three fills: gold copy offset to the light, cool copy to the other side, the ink.",
                   "Hood with a darker opening; two gold glints are the face; gold headphones over it.",
                   "Coat: an open body and two tails at the sides that swing late.",
                   "Sneakers with gold soles (they flash when a foot lifts).",
                   "Arm wave: forward kinematics, a pulse running elbow → shoulder → shoulder → elbow."],
})

INFO["glissando"] = {
    "letter": "G", "title": "Glissando — the figure of light",
    "idea": ("Neon-futuristic, our own. A near-black suit defined by light: lapis neon along the outer "
             "edge of each limb and down the torso's sides, short arcs at the elbows and knees, a collar and "
             "a belt line, the logo's five vermilion bars on the chest lit by the level, gold headphones, "
             "two gold glowing eyes. At night each line is a halo, a line and a pale core; on "
             "paper the line alone, sharp lapis on the dark suit. Fast hands and feet leave light "
             "trails. It tuts (geometric arm shapes snapped on the half beat). Signature: Halo — its hands "
             "draw a circle of light round its head; on the beat the circle drops to the floor as a ring."),
    "why": ["The neon wish, made ours: a periwinkle lapis (not the films' cyan), gold eyes, the logo's bars; no disc, helmet or circuit layout.",
            "Lapis light beside Síncopa's gold rim: the two dark figures of the cast read apart at a glance.",
            "Trails make speed visible: every snap of the tutting leaves an afterimage.",
            "Reads in both themes: lines carry it at night, the dark suit carries it on paper."],
    "risk": "Glow costs: the halo layer is translucent (see the note for its cost); on paper it is a different look (no halo).",
}
PALETTES["glissando"] = [("Suit", "#0B0E12", "#0B0E12"), ("Neon line (lapis)", "#8FAAF5", "#5B79D6"),
                         ("Core", "#E6EEFF", "#5B79D6"), ("Halo (24 %, night)", "#6F8FE8", "#F4F6F8"),
                         ("Eyes, headphones (gold)", "#F2C766", "#9C711C"), ("Chest bars", "#F0784A", "#F0784A")]
CONSTRUCTION["glissando"] = ["About 8 heads; a near-black suit #0B0E12, every piece one fill.",
                             "Neon: halo 3.2× the line at 24 % (night only), the line, a core 0.38× (night only).",
                             "Lines on each limb's outer edge (0.009 in from the edge), arcs on joints, collar, belt.",
                             "Chest: five vermilion bars, their height the level; eyes two gold slits; gold headphones.",
                             "Trails: hands and feet over the last 0.21 beat, six segments fading.",
                             "Halo: a circle r 0.25 round the head drawn by the hands, then dropped to a floor ring."]


INFO.update({
    "eco": {"letter": "H", "title": "Eco — the delay's ghost",
            "idea": ("A small floating ghost and its echoes. A dome, a scalloped hem that ripples, two arm nubs, "
                     "little gold headphones and the family's cut eyes; it wears the theme's ink (pale at night, "
                     "graphite on paper). Behind it two or three echoes repeat each move a quarter beat late — "
                     "the delay's taps — each one value further into the room. Signature: Feedback — the echoes "
                     "multiply in a widening spiral until they fill the frame, then all go out at once on the beat."),
            "why": ["The delay you can see: the echoes are the same dance, later — pure fun with a musical idea.",
                    "Opaque echoes (tints toward the room), not transparency: cheap and crisp.",
                    "A tiny silhouette that reads at 48 px: a dome and two eyes.",
                    "Cut eyes and theme ink: Compás's and Onda's family."],
            "risk": "The cutest of the cast; its charm is the echoes — judge it moving."},
    "vinilo": {"letter": "I", "title": "Vinilo — the DJ",
               "idea": ("A turntable that plays itself. Its head is a record — vinyl grooves, a vermilion label with "
                        "its face, the gold spindle for a nose, a sheen and a gold marker that turn — with DJ "
                        "headphones, one cup on. A graphite plinth body with a lapis light strip and a gold chain. "
                        "Its right arm is the tonearm (gold pivot, rod, headshell, gold stylus); its left glove "
                        "scratches the record's edge. Signature: Backspin — it brakes the record, spins it back in "
                        "a blur, and lands on the drop with the record running, arms up."),
               "why": ["A DJ made of a turntable: the scratch is literally its head moving.",
                       "The tonearm as an arm is a silhouette nobody else has.",
                       "No maker's look: no plinth layout, logo or label design of a brand.",
                       "Cheap: circles and rings; the spin is a turning sheen."],
               "risk": "The record face is small at 48 px; at 200 px the label face reads."},
    "cinta": {"letter": "J", "title": "Cinta — the cassette",
              "idea": ("An 80s cassette that breakdances. A graphite shell with a brand-free cream label and two retro "
                       "stripes, two reels for eyes that turn as it dances, the tape opening for a mouth, a backwards "
                       "lapis cap, lapis track pants with a vermilion stripe, white gloves, chunky sneakers. Its tape "
                       "slips out and swirls with every move. Signature: Rewind — the dance runs backwards at ×8, "
                       "the reels spin back, the tape whips, a gold ◀◀ flashes; then a freeze, and it restarts."),
              "why": ["Retro and street at once: toprock, a backwards cap, track pants.",
                      "The tape is secondary motion that makes every move look bigger.",
                      "Rewind is a joke only a cassette can tell.",
                      "A landscape box head — a new silhouette in the cast."],
              "risk": "Cassettes are nostalgic to some and unknown to others; the reels-as-eyes must carry it."},
    "metronomo": {"letter": "K", "title": "Metrónomo — the serious one",
                  "idea": ("A wooden metronome with a butler's dignity: a tall pyramid in two values of wood with a "
                           "gold scale and finial, the pendulum swinging in front and never off the beat, a white wing "
                           "collar and black bow tie, a gold monocle on a chain, half-lidded eyes, a curled moustache, "
                           "white gloves, a silver tray, white spats. It barely moves — a small bow on the one. "
                           "Signature: Doble tiempo — the pendulum runs at ×2, ×4 (still on the beats), he loses his "
                           "composure, the monocle drops; on the beat he straightens his tie and recomposes."),
                  "why": ["Comedy by contrast: the one who does not dance, until the pendulum runs away.",
                          "The pendulum keeps exact time: it shows the tempo better than any meter.",
                          "Wood joins the cast's palette (round 39 kept the wooden unit faces).",
                          "Not Tic (the old robot metronome): wood, manners, a monocle."],
                  "risk": "A face high on a narrow pyramid is small; the monocle and moustache carry it."},
    "espectro": {"letter": "L", "title": "Espectro — the analyser",
                 "idea": ("A figure made of spectrum bars; its body is the music. Each part is a small analyser lit "
                          "from its bottom: the legs the low frequencies (the kick), the torso the mids, the arms the "
                          "snare, the head the highs. The segments light lapis, then gold, then vermilion near the top, "
                          "with gold peak-hold caps; the unlit segments are the room's well, so the silhouette always "
                          "shows; the eyes and mouth are cut through the head's bars. Signature: Pico — every bar drops, "
                          "climbs, hits the top and breaks into gold sparks on the beat."),
                 "why": ["The visualiser as a character: in the app the bars read the real audio.",
                         "A picture of the mix: the legs pump with the kick, the head flickers with the hats.",
                         "The logo's colours as an analyser's (lapis, gold, vermilion).",
                         "Different from every other: a body made of a meter."],
                 "risk": "Its cost depends on the bar count; see the note (it must be one batched path per colour)."},
})
PALETTES.update({
    "eco": [("Ghost — ink", "#DCE3EA", "#38444F"), ("Shade", "#AEB8C2", "#1F2730"), ("Echo 1 (toward the room)", "#9AA3AD", "#8C969F"),
            ("Gold — headphones", "#E2B85A", "#9A6E1C"), ("Cut eyes — the room", "#1D2329", "#F4F6F8")],
    "vinilo": [("Vinyl", "#0D1014", "#0D1014"), ("Rim — holds 3:1", "#76818D", "#76818D"), ("Label", "#E0693B", "#E0693B"),
               ("Plinth", "#2B3239", "#2B3239"), ("Tonearm", "#B4BDC6", "#B4BDC6"), ("Light strip", "#8FAAF5", "#5B79D6"),
               ("Gold — spindle, chain, pivot", "#E2B85A", "#9A6E1C"), ("Glove", "#F3F5F7", "#F3F5F7")],
    "cinta": [("Shell", "#2B3239", "#2B3239"), ("Rim", "#76818D", "#76818D"), ("Label", "#EDE6DA", "#EDE6DA"),
              ("Stripe — vermilion", "#E0693B", "#E0693B"), ("Cap, pants — lapis", "#4664C0", "#4664C0"),
              ("Tape", "#8A5A32", "#8A5A32"), ("Reels", "#F3F5F7", "#F3F5F7"), ("Gold — laces, rewind", "#E2B85A", "#9A6E1C")],
    "metronomo": [("Wood", "#9A6A44", "#9A6A44"), ("Wood shade", "#734B2E", "#734B2E"), ("Rim", "#C9A27A", "#5E3D25"),
                  ("Scale panel", "#5E3D25", "#5E3D25"), ("Pendulum", "#C3CBD3", "#C3CBD3"), ("Gold — weight, monocle", "#E2B85A", "#E2B85A"),
                  ("Collar, gloves", "#F3F5F7", "#F3F5F7"), ("Ink — tie, moustache", "#14191E", "#14191E")],
    "espectro": [("Unlit well", "#323B44", "#D6DDE3"), ("Low — lapis", "#6A86DA", "#4664C0"), ("Mid — gold", "#E2B85A", "#9C711C"),
                 ("High — vermilion", "#F0784A", "#D2541A"), ("Peak caps", "#F2C766", "#7A5A04")],
})
CONSTRUCTION.update({
    "eco": ["A ghost 0.34 wide: a dome r 0.17, a hem of 7 scallops rippling with the beat.",
            "Echoes: the pose at t − 0.25k (k = 1…3), offset 0.085k behind, tinted 28/48/68 % toward the room.",
            "Feedback: 3 → 13 copies on a widening spiral, then none (a gold flash ring).",
            "Cut eyes and mouth in the room's colour; gold headphones.",
            "Two values per ghost (the rig's hard cel); arm nubs 0.05 wide.",
            "Floats 0.10 above its shadow; the shadow shrinks as it rises."],
    "vinilo": ["Head: a record r 0.19 with a rim, 4 groove rings, a turning sheen and marker.",
               "Label r 0.098 (vermilion, two values) with the face; a gold spindle for a nose.",
               "Plinth 0.26 wide with a lapis strip; a gold chain with a record pendant.",
               "Tonearm: gold pivot, counterweight, rod 0.020, headshell with a gold stylus.",
               "Scratch: the record's turn = 0.55 turn a beat + a rock of ±0.10 turn twice a beat.",
               "Backspin: brake, three turns back (a blurred sheen), forward on the drop."],
    "cinta": ["Cassette 0.42 × 0.27: rim, shell, a cream label, two stripes, four screws.",
              "Window 0.27 × 0.09 with two reels: hubs r 0.036, six teeth turning.",
              "The tape: a 26-point ribbon from the shell's bottom, curls following the body late.",
              "Backwards cap (two values) with a gold button; lapis track pants, vermilion stripe.",
              "Rewind: the toprock at −8× speed, the reels backwards, a gold rewind sign.",
              "Toprock: a cross-step on each beat, arms flung open."],
    "metronomo": ["A wooden pyramid 0.32 → 0.16 wide, 0.66 tall, on a plinth; gold finial.",
                  "Pendulum: a 0.37 rod from a pivot low on the front, a gold weight at 0.25.",
                  "The swing: 30° × cos(π·beat) — each extreme exactly on a beat.",
                  "Face high on the front: half-lidded eyes, a monocle on a chain, a curled moustache.",
                  "Wing collar and black bow tie; white gloves; a silver tray with a cup.",
                  "Doble tiempo: ×2 then ×4 (still on the beats), monocle falls, recomposes on the beat."],
    "espectro": ["Parts: legs (low), torso (mid), arms (snare), head (high) — each a small analyser.",
                 "Bars 0.020 wide with 0.008 gaps, segments every 0.022; lit from the part's bottom.",
                 "Lapis to 62 %, gold to 86 %, vermilion above; a gold peak-hold cap per bar.",
                 "The unlit segments are the well's colour, so the silhouette always shows.",
                 "Eyes and mouth cut through the head's bars (the room's colour).",
                 "Pico: every bar to the top, then six sparks per part, flying and falling."],
})


INFO.update({
    "onda_o1": {"letter": "E", "title": "Onda — the oscilloscope (chosen)",
                "idea": ("A figure drawn by a scope's phosphor trace — no body at all. Every limb is a line rippling "
                         "with the waveform, its amplitude the level; the head is a Lissajous loop turning in depth, "
                         "two bright dwell points for eyes. Amber phosphor (the cast's gold): a halo at night, a sharp "
                         "dark-amber line on paper. A bright beam dot runs along the trace, drawing it, and a faint "
                         "afterglow stays behind. Signature: Square — the ripples snap into a square wave and the poses "
                         "go angular (pop and lock); then a saw; then a sine."),
                "why": ["The waveform is literally its body: you see the sine, the square, the saw on every limb.",
                        "A Lissajous head is the oscilloscope's own picture — nobody else's mascot.",
                        "Light as drawing, warm amber: calm in the room, alive with the music.",
                        "Cheap: about 9 traced lines and one loop."],
                "risk": "A line figure is thin at 48 px; its charm is the trace moving (judge the GIFs)."},
    "onda_o2": {"letter": "E · O2", "title": "Onda, the wave-jelly",
                "idea": ("A chunky, wobbly creature whose top edge is the live waveform travelling across it, a gold "
                         "trace just inside that edge like a scope line on its forehead. Two values of lapis, big eyes, "
                         "a wide mouth, stubby arms and feet; it jiggles, the sides bulging on the beat. Signature: "
                         "Shape shift — the top snaps square (a crown of crenellations, eyes wide, arms up), then saw "
                         "(a row of shark teeth and a wicked grin), then melts back into a sine with a jiggle."),
                "why": ["The most fun of the three: the shape changes are jokes (a crown, shark teeth).",
                        "A solid silhouette that reads at 48 px, with a face that acts.",
                        "The waveform stays the idea: its outline is the sound.",
                        "Simple to draw: one outline path, the face, four stubs."],
                "risk": "A cute blob; it is the friendly answer, not the cool one."},
    "onda_o3": {"letter": "E · O3", "title": "Onda, the surfer",
                "idea": ("A surfer riding its own waveform. The floor is a lapis swell with a gold waveform crest, "
                         "rolling with the beat; the surfer rides it on a cream board with a vermilion stripe and a gold "
                         "fin, the board riding the wave's height and tilting with its slope. Laid-back cool: "
                         "sunglasses, messy hair with gold tips, a cream tank, vermilion board shorts, a gold pendant. "
                         "Signature: Square set — the wave turns square and it hops each step; then a saw: a slide down "
                         "the ramp, an aerial with the board turning under its feet, the landing on the drop."),
                "why": ["A cool human with a reason to move: the music literally becomes its wave.",
                        "Every shape change is a trick: steps to hop, a ramp to launch from.",
                        "A wide silhouette (the wave) that reads at any size, a figure on top.",
                        "Uses the human rig the cast already needs."],
                "risk": "The wave takes the stage's floor; on a narrow floating window it must stay inside the box."},
})
PALETTES.update({
    "onda_o1": [("Phosphor (amber)", "#F2C766", "#9C711C"), ("Core", "#FFF4D6", "#9C711C"),
                ("Halo (22 %, night)", "#E2B85A", "#F4F6F8"), ("Beam dot", "#FFF4D6", "#5E4210")],
    "onda_o2": [("Body (lapis)", "#6A86DA", "#3550A8"), ("Shade", "#4A64B8", "#24397A"), ("Trace (gold)", "#F2C766", "#E2B85A"),
                ("Eyes", "#FFFFFF", "#FFFFFF"), ("Mouth, pupils", "#14191E", "#14191E"), ("Tongue", "#E0693B", "#E0693B")],
    "onda_o3": [("Sea (lapis)", "#4664C0", "#3550A8"), ("Crest", "#6A86DA", "#5B79D6"), ("Foam (gold)", "#F2C766", "#E2B85A"),
                ("Board", "#EDE6DA", "#EDE6DA"), ("Shorts", "#E0693B", "#E0693B"), ("Skin", "#B97A57", "#B97A57")],
})
CONSTRUCTION.update({
    "onda_o1": ["About 8 heads; the figure is 9 traced lines (legs, spine, shoulders, arms, hips).",
                "Ripple: wavelength 0.075, amplitude 0.006 + 0.016·level (× 1.6 when square).",
                "Head: Lissajous x = r sin(3a + δ), y = r sin(2a), r 0.07, δ turning with the beat.",
                "Glow: halo 3.4× at 22 % (night), line, core 0.35×; paper: the line alone.",
                "Beam dot: travels the whole trace once every 1.25 beats.",
                "Afterglow: the skeleton 0.06 beat ago at 16 % (night)."],
    "onda_o2": ["Body 0.46 × 0.56; the top edge is 2 cycles of the waveform, travelling.",
                "Square: the top becomes crenellations (a crown); saw: ramps (shark teeth in the mouth).",
                "A gold trace 0.04 inside the top edge; two values of lapis.",
                "Eyes 0.048 × 0.058 with pupils; a mouth that opens; stubby arms 0.07 wide.",
                "Squash and stretch on the beat; the sides bulge with the accent.",
                "Little feet in deep lapis; a contact shadow."],
    "onda_o3": ["The sea: 0.96 wide, a swell of 1.2 cycles, amplitude 0.075 + 0.02·accent.",
                "The board: 0.46 × 0.04, riding the wave's height, tilted by its slope (≤ 28°).",
                "Square: the board sits on the steps; it hops each half beat.",
                "Saw: a slide, then an aerial (the board turns under it: a flip of its width).",
                "Surfer: about 6 heads, side stance on the board, arms out for balance.",
                "Cool face: sunglasses with a gold glint; messy hair with gold tips."],
})
