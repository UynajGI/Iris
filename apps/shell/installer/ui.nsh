; SPDX-License-Identifier: GPL-3.0-or-later
; Appended after MUI2 pages declare their controls. No installation logic here.
Var IrisHeadingFont
Var IrisBodyFont
Var IrisSmallFont
Var IrisBoldFont

!macro IrisLabel X Y W H TEXT COLOR FONT
  ${NSD_CreateLabel} ${X} ${Y} ${W} ${H} "${TEXT}"
  Pop $0
  SetCtlColors $0 "${COLOR}" "242424"
  SendMessage $0 ${WM_SETFONT} ${FONT} 1
!macroend

Function IrisFonts
  StrCmp $IrisHeadingFont "" 0 done
  CreateFont $IrisHeadingFont "Microsoft YaHei UI" 18 600
  CreateFont $IrisBodyFont "Microsoft YaHei UI" 9 400
  CreateFont $IrisSmallFont "Microsoft YaHei UI" 8 400
  CreateFont $IrisBoldFont "Microsoft YaHei UI" 10 600
  done:
FunctionEnd

Function IrisWelcomeShow
  Call IrisFonts
  ShowWindow $mui.WelcomePage.Image ${SW_HIDE}
  ShowWindow $mui.WelcomePage.Title ${SW_HIDE}
  ShowWindow $mui.WelcomePage.Text ${SW_HIDE}
  !insertmacro IrisLabel 18u 10u 290u 12u "$(IrisEyebrow)" "BDBDBD" $IrisSmallFont
  !insertmacro IrisLabel 18u 29u 290u 27u "$(IrisHero)" "EDEDED" $IrisHeadingFont
  !insertmacro IrisLabel 18u 59u 290u 14u "$(IrisIntro)" "BDBDBD" $IrisBodyFont
  !insertmacro IrisLabel 18u 77u 182u 15u "$(IrisBrowse)" "EDEDED" $IrisBoldFont
  !insertmacro IrisLabel 18u 92u 182u 12u "$(IrisBrowseBody)" "BDBDBD" $IrisSmallFont
  !insertmacro IrisLabel 18u 105u 182u 15u "$(IrisChoose)" "EDEDED" $IrisBoldFont
  !insertmacro IrisLabel 18u 120u 182u 12u "$(IrisChooseBody)" "BDBDBD" $IrisSmallFont
  !insertmacro IrisLabel 18u 133u 182u 15u "$(IrisAnalyze)" "EDEDED" $IrisBoldFont
  !insertmacro IrisLabel 18u 148u 182u 12u "$(IrisAnalyzeBody)" "BDBDBD" $IrisSmallFont
  !insertmacro IrisLabel 18u 167u 290u 11u "$(IrisSteps)" "BDBDBD" $IrisSmallFont
  !insertmacro IrisLabel 18u 181u 290u 11u "$(IrisScope)" "BDBDBD" $IrisSmallFont

  ; An abstract contact sheet, not stock or user photographs.
  ${NSD_CreateLabel} 210u 77u 97u 83u ""
  Pop $0
  SetCtlColors $0 "EDEDED" "303030"
  ${NSD_CreateLabel} 217u 85u 38u 40u ""
  Pop $0
  SetCtlColors $0 "EDEDED" "777777"
  ${NSD_CreateLabel} 259u 85u 40u 18u ""
  Pop $0
  SetCtlColors $0 "EDEDED" "595959"
  ${NSD_CreateLabel} 259u 107u 40u 18u ""
  Pop $0
  SetCtlColors $0 "EDEDED" "777777"
  ${NSD_CreateLabel} 217u 129u 82u 2u ""
  Pop $0
  SetCtlColors $0 "7563AD" "7563AD"
  ${NSD_CreateLabel} 217u 135u 82u 12u "$(IrisPreviewTitle)"
  Pop $0
  SetCtlColors $0 "EDEDED" "303030"
  SendMessage $0 ${WM_SETFONT} $IrisSmallFont 1
  ${NSD_CreateLabel} 217u 148u 82u 10u "$(IrisPreviewNote)"
  Pop $0
  SetCtlColors $0 "BDBDBD" "303030"
  SendMessage $0 ${WM_SETFONT} $IrisSmallFont 1
FunctionEnd

Function IrisFinishShow
  Call IrisFonts
  SendMessage $mui.FinishPage.Title ${WM_SETFONT} $IrisHeadingFont 1
  SendMessage $mui.FinishPage.Text ${WM_SETFONT} $IrisBodyFont 1
FunctionEnd
