REM Exercises pregap handling: track 1 has a 2 s pregap (HTOA),
REM track 2 has a 1 s pregap, track 3 has none
FILE "pregap.bin" BINARY
  TRACK 01 AUDIO
    INDEX 00 00:00:00
    INDEX 01 00:02:00
  TRACK 02 AUDIO
    INDEX 00 00:04:00
    INDEX 01 00:05:00
  TRACK 03 AUDIO
    INDEX 01 00:07:00
