# R65 assembly nyelvi kézikönyv

[English version](R65-Manual.md)

Az R65 egy KickAssembler kompatibilis 6502 keresztassembler Commodore 64-re. Ez a kézikönyv azt a nyelvet írja le, amelyet
az R65 elfogad: hogyan kell forrásfájlt írni, mit csinálnak a direktívák, mi van a script könyvtárban, és milyen fájlokat
készít az R65. Azt írja le, amit az R65 ma csinál. Ahol ez eltér a Kick Assembler kézikönyvétől, azt a
[22. fejezet](#22-korlátok) jelzi. Azt, hogy a szabályok hogyan származnak a KickAss 5.25-ből, a
[MECHANICS.md](../MECHANICS.md) írja le.

A kézikönyv összes assembly forrásként jelölt példáját lefordítják az automatikus tesztek.

## Tartalom

1. [Az első program](#1-az-első-program)
2. [A parancssor](#2-a-parancssor)
3. [A forrásfájl felépítése](#3-a-forrásfájl-felépítése)
4. [Értékek és kifejezések](#4-értékek-és-kifejezések)
5. [Utasítások és címzési módok](#5-utasítások-és-címzési-módok)
6. [Címkék és a programszámláló](#6-címkék-és-a-programszámláló)
7. [Menetek és előre hivatkozás](#7-menetek-és-előre-hivatkozás)
8. [Adatdirektívák](#8-adatdirektívák)
9. [Változók, konstansok, enumok és struktúrák](#9-változók-konstansok-enumok-és-struktúrák)
10. [Vezérlési szerkezetek](#10-vezérlési-szerkezetek)
11. [Függvények, makrók és pszeudoparancsok](#11-függvények-makrók-és-pszeudoparancsok)
12. [Hatókörök és névterek](#12-hatókörök-és-névterek)
13. [Az előfeldolgozó](#13-az-előfeldolgozó)
14. [Script könyvtár](#14-script-könyvtár)
15. [Szövegkódolások](#15-szövegkódolások)
16. [Memóriablokkok és szegmensek](#16-memóriablokkok-és-szegmensek)
17. [Kimeneti fájlok és lemezképek](#17-kimeneti-fájlok-és-lemezképek)
18. [Adatok importálása](#18-adatok-importálása)
19. [Hibakeresés és tesztelés](#19-hibakeresés-és-tesztelés)
20. [Processzorok és utasításkészletek](#20-processzorok-és-utasításkészletek)
21. [Hibaüzenetek](#21-hibaüzenetek)
22. [Korlátok](#22-korlátok)
23. [Direktívák jegyzéke](#23-direktívák-jegyzéke)

## 1. Az első program

Mentsd el ezt `border.asm` néven. A program BASIC-ből `RUN` paranccsal indul, és pirosra állítja a keretet:

```asm
BasicUpstart2(start)        // egy "10 SYS 2064" BASIC sor $0801-en

.const BORDER = $d020

* = $0810 "Main"
start:
    lda #RED
    sta BORDER
    rts
```

Fordítsd le:

```bash
r65 border.asm -showmem
```

Az R65 a forrás mellé írja a `border.prg` fájlt, és kiírja a memóriatérképet:

```text
Memory Map
----------
Default-segment:
  $0801-$080c Basic
  $080e-$080d Basic End
  $0810-$0815 Main
```

A programot VICE-ban az `x64sc border.prg` paranccsal töltheted be, vagy minden sikeres fordítás után automatikusan is
elindíthatod:

```bash
r65 border.asm -execute x64sc
```

## 2. A parancssor

```text
r65 <fájl.asm> [kapcsolók]
```

A kilépési kód sikernél 0, bármilyen hibánál 1. A sikertelen `.assert` nem változtatja meg a kilépési kódot.

| Kapcsoló | Jelentés |
|---|---|
| `-o <fájl>` | A kimeneti fájl neve, az aktuális mappához képest |
| `-odir <mappa>` | A kimeneti fájlok mappája (alapból a forrásfájl mappája) |
| `-binfile` | Nyers `.bin` fájl a 2 bájtos kezdőcím nélkül |
| `-mbfiles` | Memóriablokkonként egy fájl |
| `-nooutput` | A napló a szokásos, de nem ír fájlokat |
| `-showmem` | Kiírja a memóriatérképet |
| `-vicesymbols` | VICE címkefájlt ír (`.vs`) |
| `-symbolfile` | Importálható szimbólumfájlt ír (`.sym`) |
| `-symbolfiledir <mappa>` | Ugyanez ebbe a mappába |
| `-bytedump` | `ByteDump.txt` fájlt ír címekkel, bájtokkal és forrással |
| `-bytedumpfile <fájl>` | Ugyanez ezzel a fájlnévvel |
| `-debugdump` | `.dbg` fájlt ír a C64 Debuggerhez, amely a bájtokat a forrássorokhoz köti |
| `-asminfo <szakaszok>` | Fordítási információt ír a szerkesztőknek (lásd lent) |
| `-asminfofile <fájl>` | Az `-asminfo` fájlja; alapból `asminfo.txt` |
| `-asminfotostdout` | Fájl helyett kiírja az `-asminfo` szövegét |
| `-define <név>` | Előfeldolgozó szimbólumot definiál |
| `-libdir <mappa>` | További mappa az importált fájlok kereséséhez (ismételhető) |
| `-fillbyte <n>` | A memóriablokkok közti rések kitöltő bájtja (alapból 0) |
| `-maxaddr <n>` | Elfogadja, de nem érvényesíti, ahogy az eredeti 5.25 sem |
| `-aom` | Engedi a memóriablokkok átfedését; a későbbi blokk nyer |
| `-afo` | Engedi, hogy a script a `createFile` függvénnyel fájlt írjon |
| `-excludeillegal` | Nem fogadja el az illegális 6502 utasításokat |
| `-dtv` | Elfogadja a DTV utasításokat |
| `-cpu <név>` | `_6502NoIllegals`, `_6502`, `dtv` vagy `_65c02` |
| `-warningsoff` | Nem ír ki figyelmeztetéseket |
| `-noeval` | Csak feldolgozza a forrást |
| `-time` | Kiírja a fordítás idejét |
| `-debug` | Elfogadja, de nem ír ki többet, ahogy a kipróbált esetekben az eredeti sem |
| `-log <fájl>` | A naplót fájlba is írja |
| `-execute <parancs>` | Sikeres fordítás után elindít egy programot a kimeneti fájllal |
| `-executelog <fájl>` | Ebbe a fájlba irányítja az `-execute` programjának kimenetét. Az R65 nem várja meg a programot |
| `-cfgfile <fájl>` | További kapcsolókat olvas egy fájlból, a forrásfájlhoz képest (ismételhető). A hiányzó fájl csak figyelmeztetést ad |
| `-pseudoc3x` | Pontosvessző választja el a pszeudoparancs argumentumait, mint a Kick Assembler 3-ban |
| `-replacefile <a> <b>` | A `b` fájlt használja, bárhol az `a` fájlt importálják |
| `-maxdiag <n>` | Hány figyelmeztetést listázzon (alapból 5) |
| `:név=érték` | Érték a `cmdLineVars` script hashtáblába |
| `--gui [fájl]` | Megnyitja a szerkesztőablakot |
| `--screenshot <png>` | Megnyitja az ablakot, képet ment róla, és kilép |
| `-h`, `--help [téma]` | Súgó; az `r65 --help` felsorolja a témákat |

A kapcsolók számai írhatók `255`, `$ff` vagy `0xff` alakban.

Az `-asminfo` `|` jellel összekapcsolt szakaszneveket kap: `version`, `libraries`, `directives`, `preprocessorDirectives`,
`files`, `syntax` és `errors`, vagy az `all`, `allPredefined` és `allSourceSpecific` csoportokat. Minden szakasz a nevével
kezdődik szögletes zárójelben, a soraiban pedig `;` választja el a mezőket. Ismeretlen névre `Invalid asminfo type: x`
figyelmeztetést ad. A `[files]` csak az `errors` és `syntax` szakaszokban használt fájlokat sorolja fel. A `[version]` sorban az
R65 verziója áll, a direktívák szövegei pedig az R65 sajátjai. A `-nooutput` mellett csak az `-asminfotostdout` ad eredményt.

Az alapértelmezett kapcsolók a program melletti `r65.cfg` fájlba írhatók, soronként egy vagy több. A `#` és `//` kezdetű
sorokat kihagyja. A fájlt a parancssor előtt olvassa be, ablakos módban nem használja.

## 3. A forrásfájl felépítése

- Az **utasítások** a sor végén vagy `;` jelnél érnek véget. Sok assemblerrel ellentétben a `;` nem megjegyzés:

  ```asm
  lda #1; ldx #2; rts
  ```

- A **megjegyzés** `// a sor végéig` vagy `/* blokk */` alakú. A blokkmegjegyzés több soros is lehet, és állhat egy utasítás
  közepén is, például `sta /* keret */ $d020`.
- A **kis- és nagybetű** mindenhol különbözik. Az utasításneveket kisbetűvel kell írni: a `lda` utasítás, az `LDA`-t viszont
  ismeretlen pszeudoparancsként jelzi.
- Az **azonosítók** betűkből, számjegyekből és `_` jelből állnak, és nem kezdődhetnek számjeggyel.
- **Sortörésnek** nincs jelentése `( )` és `[ ]` között. Minden más helyen a sortörés lezárja az utasítást, a vessző és a
  műveleti jel után is, ezért a `.byte 1,` utáni új sor szintaktikai hiba:

  ```asm
  .byte (1 +
         2), 3

  .macro drawBox(x, y,
                 width,
                 height)
  {
      .byte x, y, width, height
  }

  drawBox(1, 2,
          3, 4)
  ```

- Az `.if`, `.for` és `.while` törzse lehet `{ }` blokk vagy egyetlen utasítás. A nyitó `{` állhat a következő sorban is.

## 4. Értékek és kifejezések

### Számok

| Alak | Példa | Érték |
|---|---|---|
| Tízes | `123`, `1.5` | 123, 1.5 |
| Tizenhatos | `$ff` | 255 |
| Kettes | `%1010` | 10 |

Az `1e3` alakú írásmódot nem fogadja el. Minden szám 64 bites lebegőpontos érték, és úgy íródik ki, mint a Java double:
az egész számok tört rész nélkül, különben `1.5`, `1.0E-4`, `Infinity` és `NaN` alakban.

### Amikor egy érték bájttá válik

A `.byte`, `.word` és `.dword` hibaüzenet nélkül eldobja a tört részt, és az alsó biteket tartja meg. Így a `.byte 256`
eredménye `$00`, a `.byte -1` eredménye `$ff`, a `.byte 3/2` eredménye pedig `$01`:

```asm
.byte 256, -1, 3/2, 255.9   // 00 ff 01 ff
.word 70000                 // 70 11
```

Szöveget nem lehet `.byte`-tal kiírni, arra a `.text` való. A `.number()` megadja egy karakter vagy egy szöveg
számértékét: `'x'.number()`, `"12".number()`.

### Szövegek

| Alak | Jelentés |
|---|---|
| `"szöveg"` | Sima szöveg; a fordított perjelnek nincs külön jelentése |
| `@"szöveg\n"` | Szöveg escape kódokkal |

A `@"..."` szövegek escape kódjai: `\n`, `\t`, `\r`, `\b`, `\f`, `\\`, `\"` és `\$xx`, ez utóbbi az `xx` nyers bájtot adja a
kódolástól függetlenül.

A `+` akkor fűz össze szövegeket, ha a **bal** oldala szöveg, és ilyenkor a jobb oldalt szöveggé alakítja. Ha a bal oldalon
szám áll, az hiba, ezért üres szöveggel kell kezdeni:

```asm
.var count = 3
.print "count=" + count     // count=3
.print "" + count + " items"
```

### Karakterek

Az `'a'` egy karakter. A számértékét az aktuális [kódolásból](#15-szövegkódolások) kapja, és karakterként íródik ki. Az
alapértelmezett `screencode_mixed` kódolásban az `'a'` értéke 1:

```asm
.byte 'a'                   // 01
lda #'a'
lda #'a'+1                  // a9 02: a karakter számként is működik
.text "World" + '!'         // és szövegként is, szöveg után
```

### Logikai értékek, null és listák

A `true`, `false` és `null` értékek. A `null` kiírva `Null`. A feltételeknek valódi logikai értéknek kell lenniük: az
`.if (1)` hiba, mert a számok nem alakulnak logikai értékké.

Az `[1, 2, 3]` legalább kételemű lista, az `[x]` viszont csak csoportosítás, és `x`-et adja. Üres és egyelemű listát a
`List()` készít.

### Zárójelek

A `( )` és a `[ ]` is csoportosít, és egymásba ágyazhatók. Utasítás mellett különböznek, mert a `( )` indirekt módot is
kiválaszt:

```asm
* = $1000
lda #(2 + 5) * 2            // a9 0e
lda #[2 + 5] * 2            // a9 0e
jmp ($1000)                 // 6c 00 10: indirekt
jmp [$1000]                 // 4c 00 10: abszolút
```

### Műveletek

A legszorosabbtól a leglazábbig:

| Szint | Művelet | Megjegyzés |
|---|---|---|
| 1 | `.` `[]` `++` `--` | Tag elérése, indexelés, utótag növelés |
| 2 | `-` `+` `!` `~` `++` `--` | Egyoperandusú |
| 2 | `<` `>` | Alsó és felső bájt; lásd lejjebb |
| 3 | `*` `/` | A `/` valódi osztás: a `3/2` értéke `1.5` |
| 4 | `+` `-` | |
| 5 | `<<` `>>` | Egész számok |
| 6 | `<` `<=` `>` `>=` | |
| 7 | `==` `!=` | |
| 8 | `&` | Egész számok |
| 9 | `^` | Egész számok |
| 10 | <code>&#124;</code> | Egész számok |
| 11 | `&&` | Korán megáll |
| 12 | <code>&#124;&#124;</code> | Korán megáll |
| 13 | `? :` | |
| 14 | `=` `+=` `-=` `*=` `/=` | Értékadás |

Az előtagként írt `<` és `>` a mögöttük álló teljes aritmetikai kifejezésre vonatkozik, tehát a `<$1234+1` a `$1235` alsó
bájtja:

```asm
.byte <$1234+1, >$1234      // 35 12
.byte <($1234+1)            // ugyanaz, egyértelműen írva
```

Az `&`, `|` és `^` a C nyelvhez hasonlóan lazábban köt, mint az összehasonlítás. Írd így: `(a & 1) == 1`.

Az `==` összehasonlítás: a számok és a karakterek érték szerint, a szövegek és a karakterek szövegként hasonlítanak.
Két lista csak akkor egyenlő, ha ugyanaz a lista. Szám és szöveg sosem egyenlő.

### A `?:` kifejezés

A `feltétel ? a : b` a két érték egyikét adja, és csak azt értékeli ki. Operandusban is használható:

```asm
* = $1000
.var debug = true
inc debug ? $d020 : $d013   // ee 20 d0
.var text = 1 > 2 ? "yes" : "no"
```

### Értékadás `.eval` nélkül

Az értékadás visszaadja az értékét, így az `.eval x = y = 25` mindkettőt beállítja. Az utótagként írt `x++` a változás
előtti értéket adja vissza. Önálló utasításként az értékadáshoz `.eval` kell: a puszta `x = 2`, `x++` vagy `list.add(1)` szintaktikai hiba.

```asm
.var x = 1
.eval x = 2
.eval x++
.eval x += 3
.eval x *= 2
.print "x=" + x             // x=12
```

## 5. Utasítások és címzési módok

| Mód | Írásmód | Példa |
|---|---|---|
| Implicit, akkumulátor | | `nop`, `asl` |
| Közvetlen | `#érték` | `lda #$10` |
| Nullalap | `érték` | `lda $10` |
| Nullalap indexelt | `érték,x` `érték,y` | `lda $10,x`, `ldx $10,y` |
| Abszolút | `érték` | `lda $1000` |
| Abszolút indexelt | `érték,x` `érték,y` | `lda $1000,x` |
| Indirekt | `(érték)` | `jmp ($1000)` |
| Indexelt indirekt | `(érték,x)` | `lda ($10,x)` |
| Indirekt indexelt | `(érték),y` | `lda ($10),y` |
| Relatív | `címke` | `bne loop` |

### Nullalap vagy abszolút

Az R65 akkor használja a nullalapos alakot, ha az utasításnak van ilyen, és az érték ismert és 0 és 255 közötti, vagy a címke
egy `.zp { }` blokkból származik és belefér. A használata **után** definiált címke abszolút alakot kap, figyelmeztetéssel:

```text
Warning: Using absolute mode for zeropage argument. Try defining involved labels before this statement.
```

A nullalapos címkéket definiáld a használatuk előtt, jelöld őket `.zp` blokkal, vagy kényszerítsd a módot utótaggal.
A `.zp` blokk címkéi akkor is nullalapos alakot kapnak, ha később vannak definiálva:

```asm
* = $1000
    lda zpReg1      // a5 10
    sta zpReg2      // 85 11

* = $10 "Zero page" virtual
.zp {
zpReg1: .byte 0
zpReg2: .byte 0
}
```

### A mód kényszerítése

Az utasításnév után közvetlenül írt utótag választja ki a módot:

| Utótag | Mód | Utótag | Mód |
|---|---|---|---|
| `.z` `.zp` | Nullalap | `.a` `.abs` | Abszolút |
| `.zx` `.zpx` | Nullalap,x | `.ax` `.absx` | Abszolút,x |
| `.zy` `.zpy` | Nullalap,y | `.ay` `.absy` | Abszolút,y |
| `.im` `.imm` | Közvetlen | `.i` `.ind` | Indirekt |
| `.izx` `.izpx` | `(zp,x)` | `.izy` `.izpy` | `(zp),y` |
| `.r` `.rel` | Relatív | | |

```asm
lda.a $10       // ad 10 00
lda.z $10       // a5 10
```

A `.z` csak az érték alsó bájtját tartja meg: a `lda.z $1234` eredménye `a5 34`. Olyan utasításon, amelynek nincs nullalapos
alakja, hibát ad: a `jmp.z $1000` üzenete `'jmp' does not support mode 'zeropage'`.

### Elágazások

Egy elágazás az utána következő utasítástól −128 és 127 bájt között ér el. Ennél távolabbra hiba:

```text
Error: relative address is illegal (jump distance is too far: 200).
```

## 6. Címkék és a programszámláló

### A cím beállítása

```asm
* = $1000               // egy "Unnamed" nevű blokk
* = $2000 "Main"        // névvel ellátott blokk
*=$3000 "Vars" virtual  // címeket foglal; nem ír bájtokat
.pc = $4000 "Same"      // ugyanaz, mint a *=
```

`*=` nélkül a kód `$2000`-en indul egy `Default` nevű blokkban. Kifejezésben a `*` az aktuális cím:

```asm
* = $1000
    bne *               // önmagára ugró elágazás
.print toHexString(*)
```

### Címkék

```asm
* = $1000
start:
    jmp start
.label border = $d020   // címke megadott értékkel
    inc border
```

A címke abban a hatókörben látszik, ahol definiálták, és az azon belüli hatókörökben. Ugyanannak a címkének a kétszeri
definiálása egy hatókörben hiba. A `@név:` bárhonnan a gyökér hatókörbe definiál címkét.

### Többszörös címkék

A `!név:` akárhányszor definiálható. A `!név+` a következőt, a `!név-` az előzőt jelenti. A név lehet üres, a jel
ismétlése pedig továbbugrik:

```asm
* = $1000
!:  dex
    bne !-          // az előző !
    jmp !+          // a következő !
    nop
!:  jmp !++         // innen a második !
!:  nop
!:  nop
!loop:
    bne !loop-
```

### Címkés hatókörök

Egy `{ }` blokk, makróhívás, `.if` vagy ciklus előtt közvetlenül álló címke elnevezi a hatókört, így a benne lévő címkék
kívülről ponttal elérhetők. `.if` esetén a címkék a végrehajtott ágból származnak:

```asm
* = $1000
player: {
    x: .byte 0
    y: .byte 0
}
    lda player.x

.macro clear() {
    start: lda #0
}
first: clear()
    jmp first.start

.label io = $d000 {
    nop
}

    jmp chosen.entry
chosen: .if (true) {
    entry: lda #0
} else {
    entry: nop
}
```

A `.for` és `.while` ciklus lépései indexszel érhetők el, a ciklus előtti kódból is:

```asm
* = $1000
rows: .for (var i = 0; i < 3; i++) {
    row: .byte i
}
    lda rows[1].row
```

### Operandus címkék

Az utasítás operandusában álló címke az operandus bájtjainak címét adja, ami önmódosító kódnál hasznos:

```asm
* = $1000
    lda value:#0        // a "value" a #0 bájt címe
    inc value
    jmp target:($2000)
.word target
```

### `.pseudopc`

Máshová másolva futó kódhoz: a belső címkék a célhely címeit kapják, a bájtok viszont itt maradnak.

```asm
* = $1000
.pseudopc $c000 {
copied:
    jmp copied      // 4c 00 c0
}
```

### `.align`

Egy szám következő többszörösére lép, közben nullákat ír:

```asm
* = $1001
.align $100         // most $1100-on áll
```

## 7. Menetek és előre hivatkozás

Az R65 az egész programot többször futtatja. A napló menetenként `flex pass N` sort ír, a végén pedig `Output pass`
sort. Minden menetben a definíciója előtt használt címke az előző menetből kapja az értékét. A menetek akkor érnek véget,
amikor minden érték ismert és stabil.

- **A címkék a hatókörükön belül bárhol használhatók**, a definíciójuk előtt is. **A `.var` és `.const` csak a deklaráció után
  használható**; előtte `Unknown symbol` hibát ad.
- A címek függhetnek később definiált értékektől: a `.fill later, 0` működik, ha a `later` lejjebb van definiálva.
- A sehol sem deklarált nevet az első menet után jelzi: `Unknown symbol 'név'`.
- **A `.print` egyszer fut**, a kimeneti menetben. **A `.printnow` minden menetben ír**, ami a menetek hibakeresésénél
  segít.
- A soha be nem álló program ezzel a hibával áll le: `Made no progress and can't solve the program`.

## 8. Adatdirektívák

| Direktíva | Mit ír |
|---|---|
| `.byte a, b` (`.by`) | 8 bites értékek |
| `.word a, b` (`.wo`) | 16 bites értékek, alsó bájt elöl |
| `.dword a, b` (`.dw`) | 32 bites értékek, alsó bájt elöl |
| `.text "abc", 65` (`.te`) | Szövegek az aktuális kódolásban; számok nyers bájtként; a `null` semmit sem ír |
| `.fill n, kif` | `n` darab `kif` érték, egyenként 8 bites |
| `.fillword n, kif` | `n` darab `kif` érték, egyenként 16 bites |
| `.lohifill n, kif` | Alsó bájtok táblája, utána felső bájtok táblája |

A `.fill`, `.fillword` és `.lohifill` kifejezésében használható az `i`, amely 0-tól `n`−1-ig számol. A `.fill` és `.fillword`
kifejezése lista is lehet, ilyenkor minden `i`-re az összes elemet kiírja:

```asm
* = $1000
sine:   .fill 256, 127 + 127 * sin(toRadians(i * 360 / 256))
pair:   .fill 3, [1, 2]         // 01 02 01 02 01 02
words:  .fillword 2, $1000 + i  // 00 10 01 10
```

A `.lohifill` előtti címke `lo` és `hi` mezőt kap:

```asm
* = $1000
screen: .lohifill 25, $0400 + i * 40
    ldx #3
    lda screen.lo,x
    sta $fb
    lda screen.hi,x
    sta $fc
```

## 9. Változók, konstansok, enumok és struktúrák

```asm
.var counter = 0        // változhat
.var empty              // null értékkel indul
.const WIDTH = 40       // nem változhat
.eval counter = counter + 1
```

Konstansnak értéket adni hiba (`Can't assign to 'WIDTH' since it isn't a variable`). Egy `.const`-ban lévő lista vagy
hashtábla tartalma még változhat. A `.lock()` zárolja, és ezután a zárolt lista módosítása a `You can't modify a locked value` hibát adja. A zárolt hashtáblát az eredeti engedi módosítani, így az R65 is.

Egy név egy hatókörben egyszer definiálható, akár címke, akár `.var`, akár `.const`, akár a makró, függvény vagy pszeudoparancs
paramétere: a második definíció a `The symbol 'x' is already defined` hibát adja. Ezt az első menet előtt ellenőrzi, így a soha
nem hívott makrókra és a hamis `.if` ágakra is érvényes. Az újra megnyitott `.namespace` közös neveket használ az elsővel.

A `.var` és `.const` a saját hatókörében és az azon belüli hatókörökben látszik. A blokkon belül deklarált változó a blokk
végéig elrejti a külső, azonos nevű változót. `.var` nélkül az értékadás a külső változót módosítja.

### `.label`

A `.label név = érték` névvel ellátott címet definiál. A konstanssal ellentétben a definíciója előtt is használható, és
bekerül a szimbólum- és VICE fájlokba.

### `.enum`

Az érték nélküli elemek 0-tól számolnak. Az értékkel megadott elem nem mozdítja el a számlálót:

```asm
.enum { IDLE, RUNNING, DONE = 10, FAILED }
.byte IDLE, RUNNING, DONE, FAILED   // 00 01 0a 02
```

### `.struct`

```asm
.struct Point { x, y }
.var p = Point(1, 2)
.var q = Point()        // a mezők null értékkel indulnak
.eval p.x = 9
.byte p.x, p.y          // 09 02
.print p.getStructName() + " has " + p.getNoOfFields() + " fields"
.print p.getFieldNames().get(0) // x
.eval p.set("y", 3)             // vagy index szerint: p.set(1, 3)
.print p.get("y")               // vagy index szerint: p.get(1)
```

### `.define`

A `.define a, b { ... }` saját hatókörben futtat egy blokkot, és utána csak a felsorolt neveket teszi láthatóvá, zárolt
konstansként. A blokkban nem lehet bájtokat írni. Jó hely egy lista vagy hashtábla lépésenkénti felépítésére.

```asm
.define half, double {
    .var base = 10
    .var half = base / 2
    .var double = base * 2
}
.byte half, double      // 05 14
```

## 10. Vezérlési szerkezetek

### `.if` és `.else`

A zárójel kötelező. A feltételnek logikai értéknek kell lennie.

```asm
.const DEBUG = false

.if (DEBUG) {
    inc $d020
} else .if (2 > 1) {
    sei
} else {
    cli
}

.if (!DEBUG) nop
```

Minden ág külön hatókör.

### `.for`

A fejléc első és utolsó része lehet üres, vagy több kifejezést is tartalmazhat:

```asm
.for (var i = 0, var j = 5; i < 2; i++, j--) {
    .byte i, j          // 00 05 01 04
}
```

A fejlécben álló `var` a ciklusra korlátozott. `var` nélkül a fejléc egy külső változót használ, amely megtartja a végső
értékét:

```asm
.var n = 0
.for (n = 0; n < 3; n++) { }
.byte n                 // 03
```

### `.while`

```asm
.var w = 0
.while (w < 2) {
    .byte w
    .eval w++
}
```

### Kód kibontása

A ciklusok utasításokat is írhatnak, így a kibontott kód rövid marad a forrásban:

```asm
* = $1000
.for (var row = 0; row < 25; row++) {
    sta $0400 + row * 40,x
}
```

## 11. Függvények, makrók és pszeudoparancsok

Mindhárom használható a definíciója előtt. Az azonos nevű, de eltérő paraméterszámú makrók és függvények külön definíciók.
A makrók és függvények hívhatnak másokat és önmagukat is, 10000 szint mélységig.

### Függvények

A függvény értéket számol, és nem írhat bájtokat. `.return` nélkül `null` értéket ad.

```asm
.function square(n) {
    .return n * n
}
.function square(a, b) {
    .return a * b
}
.byte square(3), square(2, 5)   // 09 0a
```

### Makrók

A makró minden hívásnál kódot ír. Minden hívásnak saját hatóköre van, így a címkéi nem ütköznek:

```asm
* = $1000
.macro wait(n) {
    ldx #n
loop:
    dex
    bne loop
}
wait(10)
wait(20)
```

A paraméterek értékek, és a híváskor egyszer értékelődnek ki. A címkével ellátott makróhívás címkéi a
[Címkés hatókörök](#címkés-hatókörök) szerint érhetők el. Ha egy név címkeként vagy pszeudoparancsként is olvasható, a
`:név(...)` kifejezetten a makrót hívja.

### Pszeudoparancsok

A pszeudoparancsot utasításként hívjuk, az argumentumokat `:` választja el. Minden argumentum megőrzi a címzési módját:

```asm
* = $1000
.pseudocommand mov src : dst {
    lda src
    sta dst
}
mov #1 : $d020
mov $10,x : $20,y
mov ($10),y : ($20,x)
```

A hiányzó argumentumok típusa `AT_NONE`. A túl sok argumentum hiba. A hívásnak egy sorban kell maradnia. A `:név` akkor is
a pszeudoparancsot hívja, ha van ilyen nevű utasítás:

```asm
* = $1000
.pseudocommand adc a : b : target {
    clc
    lda a
    .byte $69, b.getValue()     // a valódi adc #
    sta target
}
adc #$10                        // az utasítás
:adc $20 : #1 : $21             // a pszeudoparancs
```

A törzsben egy argumentumnak két metódusa van: `.getType()` és `.getValue()`. A `CmdArgument(típus, érték)` új argumentumot
készít, például a következő cím eléréséhez:

```asm
* = $1000
.pseudocommand inc16 arg {
    inc arg
    bne !+
    inc CmdArgument(arg.getType(), arg.getValue() + 1)
!:
}
inc16 $1000
inc16 $2000,x
```

| Argumentum | `getType()` | Konstans |
|---|---|---|
| nincs | 0 | `AT_NONE` |
| `#1` | 1 | `AT_IMMEDIATE` |
| `$10`, `$1000` | −1 | `AT_ABSOLUTE` |
| `$10,x`, `$1000,x` | −2 | `AT_ABSOLUTEX` |
| `$10,y`, `$1000,y` | −3 | `AT_ABSOLUTEY` |
| `($1000)` | −4 | `AT_INDIRECT` |
| `($10),y` | 6 | `AT_IZEROPAGEY` |
| `($10,x)` | −5 | Nincs hozzá konstans, lásd [Korlátok](#22-korlátok) |

Az `AT_ZEROPAGE` (2) és `AT_IZEROPAGEX` (5) konstans létezik, de egyik argumentum sem ezt adja, így a `($10,x)` és az
`AT_IZEROPAGEX` összehasonlítása hamis.

## 12. Hatókörök és névterek

### Hatókörök

A `{ }` hatókört nyit. A benne lévő címkék és változók kívülről nem látszanak, kivéve a
[megnevezett hatókörön](#címkés-hatókörök) át:

```asm
* = $1000
{
    loop: jmp loop
}
{
    loop: jmp loop      // egy másik címke
}
```

### Névterek

```asm
* = $1000
.namespace sound {
    .label VOLUME = 15
    init: rts
    .function level() { .return VOLUME }
    .macro beep() { nop }
    beep()
}
    jsr sound.init
    lda #sound.VOLUME
```

- A címkék, a `.label`-lel készültek is, és a belső névterek kívülről `sound.név` alakban érhetők el. A `.const` és a `.var` nem:
  a `sound.X` üzenete `Variable 'X' not present on value of type 'Namespace'`. Ugyanez igaz a címkék hatóköreire.
- **A névtér függvényei és makrói csak a névtéren belül hívhatók.** A `sound.level()` üzenete `Unknown function : level`, a
  `sound.beep()` utasításként pedig szintaktikai hiba.
- Ugyanannak a névtérnek az újbóli megnyitása folytatja a névteret.
- A `{ }` nélküli `.namespace név` és a `.filenamespace név` a fájl hátralévő részére érvényes.

### A gyökér hatókör

A `@` a gyökér hatókörbe teszi a definíciót, vagy onnan olvas egy nevet: kifejezésben `@név` és `@név()`, makróhívásnál
`@név()` vagy `:@név()`. Így tehetők közzé nevek egy névtérből:

```asm
* = $1000
.namespace lib {
    .macro @publicMacro() { nop }
    .function @publicFunction() { .return 7 }
    .const @PUBLIC_CONST = 5
    .var @publicVar = 4
    .struct @PublicStruct { a }
    @publicLabel: nop
}
publicMacro()
.byte publicFunction(), PUBLIC_CONST, publicVar, <publicLabel
```

## 13. Az előfeldolgozó

Ezek a direktívák futnak először, soronként, a feldolgozás előtt. `#` jellel kezdődnek:

| Direktíva | Jelentés |
|---|---|
| `#define NÉV` | Szimbólumot definiál. Parancssorból is: `-define NÉV` |
| `#undef NÉV` | Töröl egy szimbólumot |
| `#if KIF` | Megtartja a következő sorokat, ha a `KIF` igaz |
| `#elif KIF` | Különben, ha |
| `#else` | Különben |
| `#endif` | A feltétel vége |
| `#import "fájl"` | Beilleszt egy forrásfájlt |
| `#importif KIF "fájl"` | Beilleszti, ha a `KIF` igaz |
| `#importonce` | Az aktuális fájlt csak egyszer illeszti be |

A kifejezések szimbólumokat használnak `!`, `&&`, `||`, `==`, `!=` műveletekkel és zárójelekkel. Egy szimbólum akkor igaz,
ha definiált. Ezeknek a szimbólumoknak nincs értékük, így a normál kifejezésekben nem használhatók. A parancssori
értékekhez a [`cmdLineVars`](#értékek-a-parancssorból) való.

```asm
#define DEBUG

#if DEBUG && !RELEASE
    .print "debug build"
#elif RELEASE
    .print "release build"
#else
    .print "plain build"
#endif
```

Az importált fájlokat az importáló fájl mellett, a fő forrásfájl mappájában, majd az egyes `-libdir` mappákban keresi. A
`.import source "fájl"` és az `.importonce` úgy működik, mint a `#import` és `#importonce`, de az előfeldolgozós alakok
ajánlottak.

```asm
#import "helper.asm"
#import "helper.asm"    // kimarad, ha a helper.asm-ben van #importonce
```

## 14. Script könyvtár

### Matematika

`abs` `acos` `asin` `atan` `atan2` `cbrt` `ceil` `cos` `cosh` `exp` `expm1` `floor` `hypot` `IEEEremainder` `log`
`log10` `log1p` `max` `min` `mod` `pow` `random` `round` `signum` `sin` `sinh` `sqrt` `tan` `tanh` `toDegrees`
`toRadians`

- A szögek radiánban vannak.
- A `log` a természetes alapú logaritmus.
- A `max`, `min`, `atan2`, `hypot`, `pow`, `mod` és `IEEEremainder` két argumentumot vár.
- A `random()` 0-tól 1-ig ad értéket, az 1 már nincs benne. A `setSeed(n)` beállítja a kezdőértékét.
- A `round` a felezőt felfelé kerekíti, tehát a `round(2.5)` értéke 3, a `round(-2.5)` értéke −2.
- A `mod` az első argumentum előjelét tartja meg, tehát a `mod(-7, 3)` értéke −1.

Konstansok: `PI`, `E`.

### Számból szöveg

| Függvény | Példa | Eredmény |
|---|---|---|
| `toIntString(n)` | `toIntString(7)` | `7` |
| `toIntString(n, szélesség)` | `toIntString(7, 3)` | `  7` (szóközökkel kitöltve) |
| `toHexString(n, szélesség)` | `toHexString(255, 4)` | `00ff` (nullákkal kitöltve) |
| `toBinaryString(n, szélesség)` | `toBinaryString(5, 8)` | `00000101` |
| `toOctalString(n, szélesség)` | `toOctalString(8)` | `10` |

A szélesség mindegyiknél elhagyható.

### Szövegek

| Metódus | Eredmény |
|---|---|
| `.size()` | Hossz |
| `.charAt(i)` | Az `i` helyen álló karakter |
| `.substring(a, b)` | `a`-tól `b`-ig, a `b` már nincs benne |
| `.toLowerCase()`, `.toUpperCase()` | Módosított kisbetű-nagybetű |
| `.asNumber()`, `.number()` | A szövegben lévő szám |
| `.asNumber(alap)` | Ugyanez más számrendszerben: a `"f".asNumber(16)` értéke 15 |
| `.asBoolean()` | Igaz, ha a szöveg `true` |
| `.string()` | Bármely érték szövegként |

### Listák

A `List()` üres, a `List(n)` pedig `n` darab `null` elemet tartalmaz.

| Metódus | Eredmény |
|---|---|
| `.add(a, b, ...)` | Elemeket fűz hozzá; a listát adja vissza |
| `.addAll(lista)` | Egy másik lista összes elemét hozzáfűzi |
| `.get(i)`, `lista[i]` | Az `i` helyen álló elem |
| `.set(i, v)` | Lecseréli az `i` helyen álló elemet |
| `.remove(i)` | Eltávolítja az `i` helyen álló elemet, és visszaadja |
| `.size()` | Elemek száma |
| `.sort()`, `.reverse()`, `.shuffle()` | Átrendezi a listát |
| `.lock()` | Megakadályozza a további változtatást |

A `.sort()` csak számokat tartalmazó listán működik.

```asm
.var items = List().add(3, 1, 2)
.eval items.sort()
.print "" + items           // [1,2,3]
.print "" + items.get(0)    // 1
```

### Hashtáblák

| Metódus | Eredmény |
|---|---|
| `.put(k, v, ...)` | Egy vagy több párt ad hozzá; a táblát adja vissza |
| `.get(k)` | Az érték vagy `null` |
| `.containsKey(k)` | Igaz, ha a kulcs létezik |
| `.remove(k)` | Eltávolít egy kulcsot |
| `.keys()` | A kulcsok listája |
| `.lock()` | Megakadályozza a további változtatást |

A kulcsokat szövegként hasonlítja, így a `get(1)` és a `get("1")` ugyanazt a bejegyzést találja meg.

```asm
.var colors = Hashtable().put("sky", BLUE, "grass", GREEN)
.byte colors.get("sky")     // 06
```

### Értékek a parancssorból

A `:név=érték` kapcsolók a `cmdLineVars` hashtáblát töltik fel. Az értékek szövegek:

```asm
.var level = cmdLineVars.containsKey("level") ? cmdLineVars.get("level").asNumber() : 1
.print "level " + level
```

```bash
r65 game.asm :level=3
```

### Vektorok és mátrixok

| Kifejezés | Eredmény |
|---|---|
| `Vector(x, y, z)` | Vektor |
| `.get(i)`, `.getX()`, `.getY()`, `.getZ()` | Egy koordináta |
| `v + w`, `v - w`, `-v` | Vektorműveletek |
| `v * w` | Skaláris szorzat |
| `v.X(w)` | Vektoriális szorzat |
| `v * n`, `v / n` | Skálázás |
| `Matrix()` | Egységmátrix |
| `RotationMatrix(aX, aY, aZ)` | Forgatás; a szögek radiánban |
| `ScaleMatrix(x, y, z)`, `MoveMatrix(x, y, z)` | Skálázás, eltolás |
| `PerspectiveMatrix(z)` | Perspektíva |
| `.get(sor, oszlop)` | Egy cella |
| `.set(sor, oszlop, v)` | Megváltoztatja ezt a cellát, és visszaadja a mátrixot |
| `m * n` | Mátrixszorzat |
| `m * v` | Transzformált vektor, w-vel osztva |

```asm
.var turn = RotationMatrix(0, 0, toRadians(90))
.var moved = MoveMatrix(1, 0, 0) * Vector(1, 2, 3)
.print "" + moved           // (2.0,2.0,3.0)
```

### Forrásadatok

A `getFilename()` az aktuális fájl neve, a `getPath()` a teljes útvonala, a `getNamespace()` pedig az aktuális névtér
(legfelül `<RootNS>`).

Az `asmCommandSize(opkód)` egy utasítás méretét adja meg.

### Konstansok

- Színek: `BLACK` `WHITE` `RED` `CYAN` `PURPLE` `GREEN` `BLUE` `YELLOW` `ORANGE` `BROWN` `LIGHT_RED` `DARK_GRAY`
  `GRAY` `LIGHT_GREEN` `LIGHT_BLUE` `LIGHT_GRAY`. A `GREY` írásmód is működik.
- Az alap 6502 tábla opkódjai: implicit utasításnál az utasítás neve (`NOP`), különben a név és a mód:
  `_IMM` `_ZP` `_ZPX` `_ZPY` `_IZPX` `_IZPY` `_ABS` `_ABSX` `_ABSY` `_IND` `_REL`.

```asm
* = $1000
.byte LDA_IMM, 1            // a9 01
.byte NOP                   // ea
.print "" + asmCommandSize(LDA_ABS)     // 3
```

- Argumentumtípusok: lásd [Pszeudoparancsok](#pszeudoparancsok).
- Fájlformátumok: lásd [Bináris fájlok](#bináris-fájlok).

### Szövegfájlok írása

A `createFile(név)` szövegfájlt készít a kimeneti mappában, `.write(szöveg)`, `.writeln(szöveg)` és `.writeln()`
metódusokkal. Ehhez kell az `-afo` kapcsoló, különben `File output is blocked` hibát ad.

## 15. Szövegkódolások

Az `.encoding "név"` választja ki, hogy a `.text` és a `'c'` karakterek milyen bájtokká alakulnak. Az alapértelmezés
`screencode_mixed`.

| Kódolás | Az `"aA@1"` eredménye |
|---|---|
| `screencode_mixed` | `01 41 00 31` |
| `screencode_upper` | `61 01 00 31` |
| `petscii_mixed` | `41 c1 40 31` |
| `petscii_upper` | `61 41 40 31` |
| `ascii` | `61 41 40 31` |

```asm
.encoding "petscii_mixed"
.text "Hello"
.byte 0
.encoding "screencode_mixed"
.text @"raw byte: \$ff"
```

## 16. Memóriablokkok és szegmensek

### Memóriablokkok

A memóriablokk bájtok sora egy kezdőcímmel. A `* = cím "név"` indít egyet, a `.memblock "név"` pedig az aktuális címen indít
egyet. A fő kimeneti fájl a `Default` szegmens blokkjait fűzi össze, a réseket `-fillbyte` értékkel tölti ki. A blokkok nem
fedhetik át egymást: az R65 hibát jelez, hacsak nincs megadva `-aom` vagy `allowOverlap`, ilyenkor a későbbi blokk nyer,
figyelmeztetéssel.

### Szegmensek

A szegmens memóriablokkok listája, amellyel a kód és az adat tetszőleges sorrendben írható, és külön elhelyezhető.

```asm
.segmentdef Code [start=$0810]
.segmentdef Data [startAfter="Code", align=$100]
.segmentdef Vars [start=$c000, virtual]

.file [name="game.prg", segments="Code,Data"]

.segment Code "Main"
start:
    lda table
    sta counter
    rts

.segment Data "Tables"
table: .byte 1, 2

.segment Vars "Variables"
counter: .byte 0
```

- A `.segmentdef Név [paraméterek]` szegmenst definiál. A kétszeri definiálás hiba.
- A `.segment Név` átvált egy szegmensre. A `.segment Név [paraméterek] "blokk"` első alkalommal definiálja is, és blokkot
  nyit. Vissza a főprogramhoz: `.segment Default`.
- A fő kimeneti fájl csak akkor íródik ki, ha a `Default` szegmensben vannak bájtok, vagy nincs másik szegmens.

### Szegmens paraméterek

| Paraméter | Jelentés |
|---|---|
| `start=cím` | A szegmens kezdete |
| `startAfter="név"` | Közvetlenül egy másik szegmens után kezdődik |
| `align=n` | A kezdet igazítása |
| `min=cím`, `max=cím` | Megengedett tartomány, mindkét végét beleértve. A kívül eső blokk hiba, a virtuális is, és akkor is, ha semmi nem írja ki a szegmenst |
| `fill` | A teljes `min`–`max` tartomány kitöltése; mindkettő kell hozzá |
| `fillByte=n` | A kitöltő bájt |
| `virtual` | Címek kimenet nélkül |
| `allowOverlap` | A blokkok átfedhetik egymást; a későbbi nyer |
| `hide` | Nem látszik a memóriatérképen |
| `outPrg="fájl"` | A szegmenst prg fájlba írja. A `%o` a forrásfájl nevét jelenti |
| `outBin="fájl"` | A szegmenst nyers fájlba írja. A `%o` a forrásfájl nevét jelenti |
| `segments="a,b"` | Beveszi más szegmensek blokkjait |
| `prgFiles="a,b"` | Beveszi a prg fájlokat, a saját címükön |
| `sidFiles="a,b"` | Beveszi a SID fájlok zenéjét, a saját címükön |
| `modify="BasicUpstart"` | BASIC sort ad hozzá `$0801`-en, a célja `_start=cím` (vagy `marg1=cím`) |
| `dest="szöveg"` | Elfogadja, nincs hatása |

A `.file`, a `.segmentout` és a lemez fájlbejegyzései egy ideiglenes szegmensen át adják tovább a bájtjaikat, ezért ők is
elfogadják a `segments`, `prgFiles`, `sidFiles`, `allowOverlap`, `fill`, `fillByte`, `min`, `max`, `hide`, `modify` paramétereket
és a `modify` argumentumait. Az érték nélkül írt paraméter, például a `fill`, `fill=true`-t jelent. Az átfedést egy szegmensen
belül ellenőrzi, így két szegmens használhatja ugyanazokat a címeket, amíg össze nem kerülnek.

Egy szegmens blokkjai sehova sem íródnak ki, amíg nem nevezi meg `outPrg`, `outBin`, `.file`, `.disk` vagy `.segmentout`.
A `-bytedump` ekkor is megmutatja őket.

```asm
.segmentdef Rom [start=$e000, min=$e000, max=$e00f, fill, fillByte=$ea, outPrg="rom.prg"]
.segmentdef Raw [start=$2000, outBin="raw.bin"]
.segmentdef Boot [start=$0810, modify="BasicUpstart", _start=$0810, outPrg="boot.prg"]

.segment Rom
    .byte 1
.segment Raw
    .byte 2
.segment Boot
    rts
```

Egy program foltozása úgy működik, hogy az eredetit tartó szegmenst összekapcsoljuk a változásokat tartóval, mert a későbbi
blokkok nyernek:

```asm
.segmentdef Base [prgFiles="prog.prg"]
.segmentdef Patch []
.file [name="patched.prg", segments="Base,Patch", allowOverlap]

.segment Patch
* = $3001 "New code"
    lda #$ff
```

### `.segmentout`

Más szegmensek bájtjait teszi le egyben az aktuális címre. A címkéik a saját szegmensük címeit tartják meg, így készíthető elő
máshová másolandó kód:

```asm
.segmentdef Packed [start=$2000]
.segment Packed
    .byte 1, 2, 3
.segment Default
* = $1000 "Copy"
    .segmentout [segments="Packed"]
```

## 17. Kimeneti fájlok és lemezképek

| Fájl | Mikor |
|---|---|
| `név.prg` | Alapból: 2 bájtos kezdőcím, utána a bájtok |
| `név.bin` | `-binfile` kapcsolóval |
| `név_blokk.prg` | `-mbfiles` kapcsolóval, blokkonként |
| `név.vs` | `-vicesymbols` kapcsolóval: VICE címkék, töréspontok és figyelőpontok |
| `név.sym` | `-symbolfile` kapcsolóval: másik forrásból `#import`-tal betölthető címkék |
| `ByteDump.txt` | `-bytedump` kapcsolóval: címek, bájtok és forrás |

A `név` a forrásfájl neve a kiterjesztése nélkül. A fájlok a forrás mellé kerülnek, ha nincs `-odir`.

A `.sym` fájl a gyökér névtér címkéit tartalmazza, a beágyazott címkés hatókörökkel. A névterek megjelennek benne, de üresen.
Lásd [Korlátok](#22-korlátok).

### `.file`

Kiválasztott szegmenseket ír saját fájlba. A névben a `%o` a forrásfájl nevét jelenti.

```asm
.segmentdef Data [start=$2000]
.file [name="%o_data.prg", segments="Data"]
.file [name="data.bin", type="bin", segments="Data"]
.file [name="parts.prg", segments="Data", mbfiles]

.segment Data "Table"
    .byte 1, 2, 3
```

| Paraméter | Jelentés |
|---|---|
| `name="fájl"` | A fájlnév; kötelező |
| `type="prg"` vagy `"bin"` | A kezdőcímmel vagy anélkül; alapból `prg` |
| `mbfiles` | Blokkonként egy fájl: `parts_Table.prg` |

### `.disk`

35 sávos `.d64` képfájlt ír:

```asm
.segmentdef Code [start=$0810]
.segmentdef Data [start=$2000]

.disk [filename="game.d64", name="MY GAME", id="2A"] {
    [name="LOADER", type="prg", segments="Code"],
    [name="DATA",   type="seq", segments="Data"],
    [name="SECRET", type="prg", hide, segments="Data"],
}

.segment Code
    rts
.segment Data
    .byte 1
```

| Lemez paraméter | Jelentés |
|---|---|
| `filename="x.d64"` | A képfájl; kötelező. A `%o` a forrásfájl nevét jelenti |
| `name="szöveg"` | A lemez neve; alapból `UNNAMED` |
| `id="szöveg"` | A lemez azonosítója |
| `interleave=n` | Szektorlépés; alapból 10 |
| `format="commodore"` | A `commodore` 35 sávos, a `speeddos` és a `dolphindos` 40 sávos |
| `dontSplitFilesOverDir` | A 18. sáv mindkét oldalára kerülő fájl a sáv mögé költözik |
| `storeFilesInDir` | A fájlok a 18. sáv szabad szektorait is használják, amikor a 17. sáv után odaérnek |
| `showInfo` | Kiírja minden fájl nevét, szektorban mért méretét, kezdő sávját és kezdő szektorát |

| Fájl paraméter | Jelentés |
|---|---|
| `name="szöveg"` | Fájlnév, legfeljebb 16 karakter |
| `type="prg"` | `del`, `seq`, `prg`, `usr` vagy `rel`; a záró `<` zárolja a fájlt, például `"prg<"` |
| `hide` | A lemezen van, de a könyvtárban nem látszik |
| `noStartAddr` | Kihagyja a kezdőcímet. Alapból csak a `prg` fájloknak van; a `noStartAddr=false` a többin is megtartja |
| `interleave=n` | Szektorlépés ennél a fájlnál |
| `segments`, `prgFiles`, `sidFiles` | Honnan jönnek a bájtok |

Egy lemezen legfeljebb 152 látható fájl lehet. A tele lemezre `Disk is full!` hibát ad.

## 18. Adatok importálása

### `.import`

| Direktíva | Mit ír |
|---|---|
| `.import binary "fájl", eltolás, hossz` | A fájl bájtjait |
| `.import c64 "fájl", eltolás, hossz` | Ugyanezt, a 2 bájtos kezdőcímet kihagyva |
| `.import text "fájl", eltolás, hossz` | A fájlt szövegként, az aktuális kódolásban |
| `.import source "fájl"` | Forrást illeszt be, mint a `#import` |

Az `eltolás` és a `hossz` elhagyható.

```asm
* = $1000
.import binary "data.bin"
.import binary "data.bin", 1, 2
.import c64 "data.bin"
```

### Bináris fájlok

A `LoadBinary(fájl)` egy értékbe olvas be egy fájlt, ennek metódusai a `.getSize()`, a `.get(i)` (előjeles) és az `.uget(i)`
(előjel nélküli).

```asm
* = $1000
.var data = LoadBinary("data.bin")
.fill data.getSize(), data.uget(i)
```

A második argumentum elnevezi a fájl részeit, `"C64FILE, név=eltolás, ..."` alakban. A `C64FILE` kihagyja a 2 bájtos kezdőcímet.
Minden név saját metódusokat kap:

```asm
* = $1000
.var file = LoadBinary("data.bin", "first=0, second=2")
.byte file.getfirstSize(), file.ugetsecond(0)
```

A `BF_C64FILE`, `BF_KOALA`, `BF_FLI`, `BF_BITMAP_SINGLECOLOR` és `BF_DOODLE` konstansok a gyakori formátumok sablonjai:

| Konstans | Részek |
|---|---|
| `BF_KOALA` | `Bitmap`, `ScreenRam`, `ColorRam`, `BackgroundColor` |
| `BF_FLI` | `ColorRam`, `ScreenRam`, `Bitmap` |
| `BF_BITMAP_SINGLECOLOR` | `Bitmap`, `ScreenRam` |
| `BF_DOODLE` | `ColorRam`, `Bitmap` |

Így egy Koala kép `.getBitmap(i)`, `.getBitmapSize()` és hasonló metódusokat ad.

### SID fájlok

A `LoadSid(fájl)` SID zenét olvas be. A mezői: `.header`, `.version`, `.location`, `.init`, `.play`, `.songs`,
`.startSong`, `.speed`, `.name`, `.author`, `.copyright`, `.size`, `.flags`, `.startpage` és `.pagelength`.
A `.getData(i)` a zene egy bájtját adja.

```asm
.var music = LoadSid("tune.sid")
* = music.location "Music"
.fill music.size, music.getData(i)
.print "init=$" + toHexString(music.init) + " play=$" + toHexString(music.play)
```

### Képek

A `LoadPicture(fájl)` gif, jpg vagy png képet olvas be.

| Tag | Eredmény |
|---|---|
| `.width`, `.height` | Méret pixelben |
| `.getPixel(x, y)` | Szín `$rrggbb` alakban |
| `.getSinglecolorByte(x, y)` | 8 pixel `x`-től (bájtban számolva) és `y`-tól; a bit = színindex & 1 |
| `.getMulticolorByte(x, y)` | 4 dupla pixel; 2 bit = színindex & 3 |

A színindex a színtáblából jön. Második argumentum nélkül a tábla feketével kezdődik, utána jönnek az első új színek
pixelsorrendben, legfeljebb 4 szín. A `LoadPicture(fájl, lista)` alakban a lista adja a táblát:

```asm
* = $2000
.var logo = LoadPicture("picture.png", List().add($000000, $ffffff, $ff0000))
.fill logo.width / 8, logo.getSinglecolorByte(i, 0)
```

## 19. Hibakeresés és tesztelés

| Direktíva | Jelentés |
|---|---|
| `.print kif` | A kimeneti menetben ír |
| `.printnow kif` | Minden menetben ír |
| `.error üzenet` | Hibával megáll |
| `.errorif feltétel, üzenet` | Hibával megáll, ha a `feltétel` igaz |
| `.assert "név", a, b` | Ellenőrzi, hogy `a` egyenlő `b`-vel |
| `.asserterror "név", kif` | Ellenőrzi, hogy `kif` hibát ad |
| `.break [arg]` | VICE töréspont a következő utasításnál (`-vicesymbols` kapcsolóval) |
| `.watch a [, [b] [, arg]]` | VICE figyelőpont egy címre vagy tartományra (`-vicesymbols` kapcsolóval) |

```asm
* = $1000
.assert "two", 1 + 1, 2
.asserterror "bad type", 1 + "a"
.errorif * > $2000, "the code is too long"

    nop
.break
    nop
.break "if A == 1"
.watch $d020
.watch $1000, $1010, "store"
.watch $d021, , "hex8"

    beq short
.errorif (>*) != (>short), "this branch crosses a page"
    nop
short:
```

Mindkettő kódblokkot is elfogad. Ezeket külön fordítja le, és nem kerülnek a programba. Az `.assert` a bájtjaikat hasonlítja
össze, az `.asserterror` pedig azt várja, hogy a blokk hibát adjon:

```asm
.assert "same code", { lda $1000 }, { lda $1000 }
.assert "with a loop", {
    .for (var i = 0; i < 2; i++)
        sta $0400 + i
}, {
    sta $0400
    sta $0401
}
.asserterror "must fail", { lda #"text" }
```

A napló mutatja a bájtokat: `same code -- OK. | 2000:ad,00,10`, vagy eltérés esetén `x -- FAILED! | 2000:ad,00,10 -- 2000:ae,00,10`.

Az eredmény a napló végén olvasható, `Made 2 asserts , 0 failed` alakban. **A sikertelen assert nem állítja le a fordítást,
és nem változtatja meg a kilépési kódot**, ezért nézd a naplót, vagy használj `.errorif` direktívát, ha a fordításnak el kell
buknia.

A `-vicesymbols` ezeket a sorokat írja a `.vs` fájlba:

```text
al C:1000 .lbl
break 1001 if A == 1
watch d020
watch store 1000 1010
watch hex8 d021
```

A `-bytedump` minden utasítás címét, bájtjait és forrását megmutatja, így látszik a leggyorsabban, mit ír egy makró.

## 20. Processzorok és utasításkészletek

Kiválasztás: `-cpu <név>` vagy a forrásban `.cpu <név>`.

| Név | Utasítások |
|---|---|
| `_6502` | Szabványos 6502 és illegális utasítások; ez az alap |
| `_6502NoIllegals` | Csak a szabványos 6502 (`-excludeillegal`) |
| `dtv` | C64 DTV: ezen felül `sac`, `sir` és `bra` (`-dtv`) |
| `_65c02` | 65C02 |

A táblázatok címzési módonként, tizenhatos számrendszerben adják meg a kódot. Az R65 minden kombinációra való futtatásával
készültek.

### Szabványos 6502

| Utasítás | impl | imm | zp | zp,x | zp,y | (zp,x) | (zp),y | abs | abs,x | abs,y | (ind) | rel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `adc` |  | 69 | 65 | 75 |  | 61 | 71 | 6D | 7D | 79 |  |  |
| `and` |  | 29 | 25 | 35 |  | 21 | 31 | 2D | 3D | 39 |  |  |
| `asl` | 0A |  | 06 | 16 |  |  |  | 0E | 1E |  |  |  |
| `bcc` |  |  |  |  |  |  |  |  |  |  |  | 90 |
| `bcs` |  |  |  |  |  |  |  |  |  |  |  | B0 |
| `beq` |  |  |  |  |  |  |  |  |  |  |  | F0 |
| `bit` |  |  | 24 |  |  |  |  | 2C |  |  |  |  |
| `bmi` |  |  |  |  |  |  |  |  |  |  |  | 30 |
| `bne` |  |  |  |  |  |  |  |  |  |  |  | D0 |
| `bpl` |  |  |  |  |  |  |  |  |  |  |  | 10 |
| `brk` | 00 |  |  |  |  |  |  |  |  |  |  |  |
| `bvc` |  |  |  |  |  |  |  |  |  |  |  | 50 |
| `bvs` |  |  |  |  |  |  |  |  |  |  |  | 70 |
| `clc` | 18 |  |  |  |  |  |  |  |  |  |  |  |
| `cld` | D8 |  |  |  |  |  |  |  |  |  |  |  |
| `cli` | 58 |  |  |  |  |  |  |  |  |  |  |  |
| `clv` | B8 |  |  |  |  |  |  |  |  |  |  |  |
| `cmp` |  | C9 | C5 | D5 |  | C1 | D1 | CD | DD | D9 |  |  |
| `cpx` |  | E0 | E4 |  |  |  |  | EC |  |  |  |  |
| `cpy` |  | C0 | C4 |  |  |  |  | CC |  |  |  |  |
| `dec` |  |  | C6 | D6 |  |  |  | CE | DE |  |  |  |
| `dex` | CA |  |  |  |  |  |  |  |  |  |  |  |
| `dey` | 88 |  |  |  |  |  |  |  |  |  |  |  |
| `eor` |  | 49 | 45 | 55 |  | 41 | 51 | 4D | 5D | 59 |  |  |
| `inc` |  |  | E6 | F6 |  |  |  | EE | FE |  |  |  |
| `inx` | E8 |  |  |  |  |  |  |  |  |  |  |  |
| `iny` | C8 |  |  |  |  |  |  |  |  |  |  |  |
| `jmp` |  |  |  |  |  |  |  | 4C |  |  | 6C |  |
| `jsr` |  |  |  |  |  |  |  | 20 |  |  |  |  |
| `lda` |  | A9 | A5 | B5 |  | A1 | B1 | AD | BD | B9 |  |  |
| `ldx` |  | A2 | A6 |  | B6 |  |  | AE |  | BE |  |  |
| `ldy` |  | A0 | A4 | B4 |  |  |  | AC | BC |  |  |  |
| `lsr` | 4A |  | 46 | 56 |  |  |  | 4E | 5E |  |  |  |
| `nop` | EA |  |  |  |  |  |  |  |  |  |  |  |
| `ora` |  | 09 | 05 | 15 |  | 01 | 11 | 0D | 1D | 19 |  |  |
| `pha` | 48 |  |  |  |  |  |  |  |  |  |  |  |
| `php` | 08 |  |  |  |  |  |  |  |  |  |  |  |
| `pla` | 68 |  |  |  |  |  |  |  |  |  |  |  |
| `plp` | 28 |  |  |  |  |  |  |  |  |  |  |  |
| `rol` | 2A |  | 26 | 36 |  |  |  | 2E | 3E |  |  |  |
| `ror` | 6A |  | 66 | 76 |  |  |  | 6E | 7E |  |  |  |
| `rti` | 40 |  |  |  |  |  |  |  |  |  |  |  |
| `rts` | 60 |  |  |  |  |  |  |  |  |  |  |  |
| `sbc` |  | E9 | E5 | F5 |  | E1 | F1 | ED | FD | F9 |  |  |
| `sec` | 38 |  |  |  |  |  |  |  |  |  |  |  |
| `sed` | F8 |  |  |  |  |  |  |  |  |  |  |  |
| `sei` | 78 |  |  |  |  |  |  |  |  |  |  |  |
| `sta` |  |  | 85 | 95 |  | 81 | 91 | 8D | 9D | 99 |  |  |
| `stx` |  |  | 86 |  | 96 |  |  | 8E |  |  |  |  |
| `sty` |  |  | 84 | 94 |  |  |  | 8C |  |  |  |  |
| `tax` | AA |  |  |  |  |  |  |  |  |  |  |  |
| `tay` | A8 |  |  |  |  |  |  |  |  |  |  |  |
| `tsx` | BA |  |  |  |  |  |  |  |  |  |  |  |
| `txa` | 8A |  |  |  |  |  |  |  |  |  |  |  |
| `txs` | 9A |  |  |  |  |  |  |  |  |  |  |  |
| `tya` | 98 |  |  |  |  |  |  |  |  |  |  |  |

### Illegális utasítások

A `_6502` készlet ezeket adja a szabványos táblához. Az egy sorban álló nevek egymás álnevei.

| Utasítás | impl | imm | zp | zp,x | zp,y | (zp,x) | (zp),y | abs | abs,x | abs,y | (ind) | rel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `ahx`, `sha` |  |  |  |  |  |  | 93 |  |  | 9F |  |  |
| `alr`, `asr` |  | 4B |  |  |  |  |  |  |  |  |  |  |
| `anc` |  | 0B |  |  |  |  |  |  |  |  |  |  |
| `anc2` |  | 2B |  |  |  |  |  |  |  |  |  |  |
| `arr` |  | 6B |  |  |  |  |  |  |  |  |  |  |
| `axs`, `sbx` |  | CB |  |  |  |  |  |  |  |  |  |  |
| `dcp`, `dcm` |  |  | C7 | D7 |  | C3 | D3 | CF | DF | DB |  |  |
| `isc`, `ins`, `isb` |  |  | E7 | F7 |  | E3 | F3 | EF | FF | FB |  |  |
| `las`, `lae`, `lds` |  |  |  |  |  |  |  |  |  | BB |  |  |
| `lax`, `lxa` |  | AB | A7 |  | B7 | A3 | B3 | AF |  | BF |  |  |
| `nop` | EA | 80 | 04 | 14 |  |  |  | 0C | 1C |  |  |  |
| `rla` |  |  | 27 | 37 |  | 23 | 33 | 2F | 3F | 3B |  |  |
| `rra` |  |  | 67 | 77 |  | 63 | 73 | 6F | 7F | 7B |  |  |
| `sax` |  |  | 87 |  | 97 | 83 |  | 8F |  |  |  |  |
| `sbc2` |  | EB |  |  |  |  |  |  |  |  |  |  |
| `shx` |  |  |  |  |  |  |  |  |  | 9E |  |  |
| `shy` |  |  |  |  |  |  |  |  | 9C |  |  |  |
| `slo` |  |  | 07 | 17 |  | 03 | 13 | 0F | 1F | 1B |  |  |
| `sre` |  |  | 47 | 57 |  | 43 | 53 | 4F | 5F | 5B |  |  |
| `tas`, `shs` |  |  |  |  |  |  |  |  |  | 9B |  |  |
| `xaa`, `ane` |  | 8B |  |  |  |  |  |  |  |  |  |  |

### DTV

A `dtv` készlet ezeket adja az illegális készlethez: `bra` (relatív, 12), `sac` (közvetlen, 32) és `sir` (közvetlen, 42).

### 65C02

A `_65c02` készletben nincsenek illegális utasítások. Ezek a sorok újak vagy eltérnek a szabványos táblától. A `(zp)` írásmódja
`lda ($10)`, a `zp,rel` írásmódja `bbr0 $10,címke`, az `(abs,x)` írásmódja pedig `jmp ($1000,x)`.

| Utasítás | impl | imm | zp | zp,x | zp,y | (zp,x) | (zp),y | abs | abs,x | abs,y | (ind) | rel | (zp) | zp,rel | (abs,x) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `adc` |  | 69 | 65 | 75 |  | 61 | 71 | 6D | 7D | 79 |  |  | 72 |  |  |
| `and` |  | 29 | 25 | 35 |  | 21 | 31 | 2D | 3D | 39 |  |  | 32 |  |  |
| `bbr0` |  |  |  |  |  |  |  |  |  |  |  |  |  | 0F |  |
| `bbr1` |  |  |  |  |  |  |  |  |  |  |  |  |  | 1F |  |
| `bbr2` |  |  |  |  |  |  |  |  |  |  |  |  |  | 2F |  |
| `bbr3` |  |  |  |  |  |  |  |  |  |  |  |  |  | 3F |  |
| `bbr4` |  |  |  |  |  |  |  |  |  |  |  |  |  | 4F |  |
| `bbr5` |  |  |  |  |  |  |  |  |  |  |  |  |  | 5F |  |
| `bbr6` |  |  |  |  |  |  |  |  |  |  |  |  |  | 6F |  |
| `bbr7` |  |  |  |  |  |  |  |  |  |  |  |  |  | 7F |  |
| `bbs0` |  |  |  |  |  |  |  |  |  |  |  |  |  | 8F |  |
| `bbs1` |  |  |  |  |  |  |  |  |  |  |  |  |  | 9F |  |
| `bbs2` |  |  |  |  |  |  |  |  |  |  |  |  |  | AF |  |
| `bbs3` |  |  |  |  |  |  |  |  |  |  |  |  |  | BF |  |
| `bbs4` |  |  |  |  |  |  |  |  |  |  |  |  |  | CF |  |
| `bbs5` |  |  |  |  |  |  |  |  |  |  |  |  |  | DF |  |
| `bbs6` |  |  |  |  |  |  |  |  |  |  |  |  |  | EF |  |
| `bbs7` |  |  |  |  |  |  |  |  |  |  |  |  |  | FF |  |
| `bit` |  | 89 | 24 | 34 |  |  |  | 2C | 3C |  |  |  |  |  |  |
| `bra` |  |  |  |  |  |  |  |  |  |  |  | 80 |  |  |  |
| `cmp` |  | C9 | C5 | D5 |  | C1 | D1 | CD | DD | D9 |  |  | D2 |  |  |
| `dec` | 3A |  | C6 | D6 |  |  |  | CE | DE |  |  |  |  |  |  |
| `eor` |  | 49 | 45 | 55 |  | 41 | 51 | 4D | 5D | 59 |  |  | 52 |  |  |
| `inc` | 1A |  | E6 | F6 |  |  |  | EE | FE |  |  |  |  |  |  |
| `jmp` |  |  |  |  |  |  |  | 4C |  |  | 6C |  |  |  | 7C |
| `lda` |  | A9 | A5 | B5 |  | A1 | B1 | AD | BD | B9 |  |  | B2 |  |  |
| `ora` |  | 09 | 05 | 15 |  | 01 | 11 | 0D | 1D | 19 |  |  | 12 |  |  |
| `phx` | DA |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `phy` | 5A |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `plx` | FA |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `ply` | 7A |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb0` |  |  | 07 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb1` |  |  | 17 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb2` |  |  | 27 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb3` |  |  | 37 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb4` |  |  | 47 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb5` |  |  | 57 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb6` |  |  | 67 |  |  |  |  |  |  |  |  |  |  |  |  |
| `rmb7` |  |  | 77 |  |  |  |  |  |  |  |  |  |  |  |  |
| `sbc` |  | E9 | E5 | F5 |  | E1 | F1 | ED | FD | F9 |  |  | F2 |  |  |
| `smb0` |  |  | 87 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb1` |  |  | 97 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb2` |  |  | A7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb3` |  |  | B7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb4` |  |  | C7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb5` |  |  | D7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb6` |  |  | E7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `smb7` |  |  | F7 |  |  |  |  |  |  |  |  |  |  |  |  |
| `sta` |  |  | 85 | 95 |  | 81 | 91 | 8D | 9D | 99 |  |  | 92 |  |  |
| `stp` | DB |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `stz` |  |  | 64 | 74 |  |  |  | 9C | 9E |  |  |  |  |  |  |
| `trb` |  |  | 14 |  |  |  |  | 1C |  |  |  |  |  |  |  |
| `tsb` |  |  | 04 |  |  |  |  | 0C |  |  |  |  |  |  |  |
| `wai` | CB |  |  |  |  |  |  |  |  |  |  |  |  |  |  |

### Példa

```asm
.cpu _65c02
* = $1000
    phx
    stz $10
    lda ($10)
    inc
    bbr0 $10, *
    jmp ($1000,x)
    bra *
.cpu _6502
    lax $10
```

## 21. Hibaüzenetek

Egy hiba legfeljebb három sort mutat a hibás sor előtt, `^` jelet az oszlop alatt, az üzenetet, a helyet, valamint makrókban
és függvényekben azt, hogy honnan hívták:

```text
.byte 256
lda nosuch
    ^

Error: Unknown symbol 'nosuch'
at line 2, column 5 in game.asm
```

| Üzenet | Szokásos ok |
|---|---|
| `Unknown symbol 'x'` | Elírt név, vagy belső hatókörben definiált név, vagy a deklaráció előtt használt `.var` |
| `The symbol 'x' is already defined` | Ugyanaz a név kétszer egy hatókörben |
| `Can't assign to 'x' since it isn't a variable` | Értékadás címkének vagy `.const`-nak |
| `Pseudo command 'x' not defined` | Nagybetűs utasításnév vagy hiányzó definíció |
| `Macro command 'x' not defined` | Rossz név |
| `Invalid number of arguments for macro` | Rossz argumentumszám |
| `Unknown function : +` | Olyan művelet vagy metódus, amely az értéknek nincs, például `"x"-1` |
| `Variable 'x' not present on value of type 'Namespace'` | Nem címke áll a pont után |
| `Missing start token`, `Missing end token` | Pár nélküli zárójel |
| `'jmp' does not support mode 'zeropage'` | `.z` olyan utasításon, amelynek nincs ilyen módja |
| `In segment 'X' the memoryblock 'A' (...) is out of segment bounds (...).` | A blokk kívül esik a `min` és `max` határon |
| `invalid adressingmode. 'x' doesn't support ...` | Az utasításnak nincs ilyen módja, például `sta #1` |
| `The mnemonic 'x' is not supported by the current cpu` | Válts processzort |
| `relative address is illegal (jump distance is too far: n).` | Használj `jmp`-t, vagy fordítsd meg az elágazást |
| `Can't get a double representation from a value of type String` | Szám áll a `+` bal oldalán és szöveg a jobb oldalán, vagy szöveg van `.byte`-ban |
| `Can't get a boolean representation from a value of type Number` | `.if (1)`; írj összehasonlítást |
| `Can't output bytes inside a function or a .define block` | Használj makrót |
| `In segment 'X' the memoryblock 'A' (...) overlaps 'B' (...)` | Két blokk ugyanazon a címen |
| `Can't open file: x`, `Can't open file 'x'` | Ellenőrizd az útvonalat és a `-libdir` kapcsolót |
| `Too deep recursion` | Önmagát a végtelenségig hívó makró vagy függvény. Az R65 saját szövege: az eredeti Java `StackOverflowError` hibával áll le |

## 22. Korlátok

- A Java bővítmények nem támogatottak: a `.plugin`, `.modify` és `.filemodify` hibát ad. Az egyetlen szegmens módosító a
  `BasicUpstart`.
- A `-maxaddr` kapcsolót elfogadja, de nem érvényesíti, ahogy az eredeti 5.25 sem.
- A `dest` szegmens paraméternek nincs hatása.
- A progresszív vagy alulmintavételezett JPEG képek kis mértékben eltérhetnek az eredetitől.

### Eltérések a Kick Assembler kézikönyvtől

Az R65 az eredeti programot, a KickAss.jar 5.25-öt követi, és mindkettő futtatásával vetettük össze. Ahol az eredeti kézikönyve
mást mond, ott az R65 azt csinálja, amit a program:

| Téma | A Kick Assembler kézikönyv | A program és az R65 |
|---|---|---|
| Sortörés | A kifejezés folytatódik a műveleti jel után | `( )` és `[ ]` kívül szintaktikai hiba |
| `jmp.z` | Megengedett | `'jmp' does not support mode 'zeropage'` |
| `AT_IZEROPAGEX` | A `($10,x)` típusa | A `($10,x)` értéke -5, és egyik argumentum sem ad 5-öt |
| `noStartAddr` | Minden fájltípusnál hamis az alapérték | Alapból csak a `prg` kap kezdőcímet |
| `.sym` és névterek | `.label name = $2000` | `.label name=$2000`, behúzva a `.namespace name { }` blokkon belül |

### Eltérések az eredeti programtól

- Több hibánál az eredeti először mindet felsorolja (`Got 2 errors and 0 warnings while executing:`). Az R65 az elsőnél megáll, és
  az eredeti is ezt írja le részletesen.
- Az eredeti az első menet előtt ellenőrzi a neveket, ezért a soha nem hívott makróban álló `lda nosuch` ott hiba. Az R65 csak a
  lefutó kódot ellenőrzi, és máshol az ismeretlen függvényt vagy a rossz argumentumszámot sem találja meg.
- A túl sok argumentummal hívott metódus az eredetiben hiba, az R65 figyelmen kívül hagyja a fölösleget.
- A végtelen rekurzió az R65 saját hibájával áll le, a 2000 szintű rekurzió pedig működik, míg az eredetinek elfogy a verme.
- A `flex pass` sorok száma eltérhet, ha egy szegmens egy másik után kezdődik, vagy ha olyan címke mozdul el, amelyet semmi nem használ.
- `-asminfo`: a `[version]` az R65 verzióját adja, a direktívák szövegei az R65 sajátjai, a `syntax` bejegyzések más sorrendben
  állnak, a beépített makrókéi pedig kimaradnak. A beépített fájl neve `r65:autoinclude.asm`.
- A `-cfgfile` fájl `//` sorát kihagyja, az eredeti figyelmeztet a szavaira.

## 23. Direktívák jegyzéke

| Direktíva | Fejezet |
|---|---|
| `.align` | [6](#align) |
| `.assert`, `.asserterror` | [19](#19-hibakeresés-és-tesztelés) |
| `.break`, `.watch` | [19](#19-hibakeresés-és-tesztelés) |
| `.byte`, `.word`, `.dword`, `.text` | [8](#8-adatdirektívák) |
| `.const`, `.var`, `.eval` | [9](#9-változók-konstansok-enumok-és-struktúrák) |
| `.cpu` | [20](#20-processzorok-és-utasításkészletek) |
| `.define` | [9](#define) |
| `.disk` | [17](#disk) |
| `.encoding` | [15](#15-szövegkódolások) |
| `.enum`, `.struct` | [9](#9-változók-konstansok-enumok-és-struktúrák) |
| `.error`, `.errorif` | [19](#19-hibakeresés-és-tesztelés) |
| `.file` | [17](#file) |
| `.fill`, `.fillword`, `.lohifill` | [8](#8-adatdirektívák) |
| `.filemodify`, `.modify`, `.plugin` | [22](#22-korlátok) |
| `.filenamespace`, `.namespace` | [12](#névterek) |
| `.for`, `.if`, `.while` | [10](#10-vezérlési-szerkezetek) |
| `.function`, `.macro`, `.pseudocommand`, `.return` | [11](#11-függvények-makrók-és-pszeudoparancsok) |
| `.import` | [18](#import) |
| `.label` | [9](#label) |
| `.memblock`, `.segment`, `.segmentdef`, `.segmentout` | [16](#16-memóriablokkok-és-szegmensek) |
| `.pc` | [6](#a-cím-beállítása) |
| `.print`, `.printnow` | [19](#19-hibakeresés-és-tesztelés) |
| `.pseudopc` | [6](#pseudopc) |
| `.zp` | [5](#nullalap-vagy-abszolút) |
| `.importonce` | [13](#13-az-előfeldolgozó) |
| `BasicUpstart`, `BasicUpstart2` | Lent |

A beépített `BasicUpstart(cím)` makró az aktuális címre írja a `10 SYS cím` BASIC sort. A `BasicUpstart2(cím)` ugyanezt
`$0801`-en teszi egy `Basic` nevű blokkban, majd megnyit egy üres `Basic End` nevű blokkot `$080e`-n.
