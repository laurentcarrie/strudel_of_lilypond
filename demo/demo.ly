\version "2.24.4"

voicea = {
  \absolute {
    \repeat percent 2 {
      <a c e>4 a4 a4 a4 |
      b4\3 r2 b4\3 |
    }
  }
    <c\5 g\4 c'\3>1 
}




mydrums = \drummode {
  \repeat percent 2 {
    bd4 sn4 bd4 sn4 |
  }
|
  \repeat percent 2 {
    bd8 bd8 r4 bd4 sn4 |
  }


    bd4 bd4 bd4 bd4 |

}

myhh = \drummode {
  \repeat unfold 5 {
    hh8 hh8 hh8 hh8 hh8 hh8 hh8 hh8 |
  }
}

\paper {
  #(include-special-characters)
  indent = 0\mm
  line-width = 180\mm
  oddHeaderMarkup = ""
  evenHeaderMarkup = ""
  oddFooterMarkup = ""
  evenFooterMarkup = ""
  #(add-text-replacements!
    '(("100" . "hundred")
      ("dpi" . "dots per inch")))
}

\score {
  <<
    \tempo 4 = 60
    \set Score.currentBarNumber = 89

    \new DrumStaff {
      <<
        \new DrumVoice {
          \voiceOne
          % @strudel-of-lilypond@ cyan punchcard
          % @strudel-of-lilypond@ pan <0 .5 1>
          % @strudel-of-lilypond@ gain <1 2 3>

          \mydrums
        }
        \new DrumVoice {
          \voiceTwo
          \myhh
        }
      >>
    }
  >>

  \layout {}
}
