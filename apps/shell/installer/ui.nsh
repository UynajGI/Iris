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
  CreateFont $IrisHeadingFont "Microsoft YaHei UI" 16 400
  CreateFont $IrisBodyFont "Microsoft YaHei UI" 9 400
  CreateFont $IrisSmallFont "Microsoft YaHei UI" 8 400
  CreateFont $IrisBoldFont "Microsoft YaHei UI" 10 400
  done:
FunctionEnd

Function IrisWelcomeShow
  Call IrisFonts
  ShowWindow $mui.WelcomePage.Image ${SW_HIDE}
  ShowWindow $mui.WelcomePage.Title ${SW_HIDE}
  ShowWindow $mui.WelcomePage.Text ${SW_HIDE}
  !insertmacro IrisLabel 16u 18u 290u 27u "$(IrisHero)" "EDEDED" $IrisHeadingFont
  !insertmacro IrisLabel 16u 45u 290u 16u "$(IrisIntro)" "BDBDBD" $IrisBodyFont
  !insertmacro IrisLabel 16u 74u 185u 15u "$(IrisBrowse)" "EDEDED" $IrisBoldFont
  !insertmacro IrisLabel 16u 90u 185u 14u "$(IrisBrowseBody)" "BDBDBD" $IrisSmallFont
  !insertmacro IrisLabel 16u 108u 185u 15u "$(IrisChoose)" "EDEDED" $IrisBoldFont
  !insertmacro IrisLabel 16u 124u 185u 14u "$(IrisChooseBody)" "BDBDBD" $IrisSmallFont
  !insertmacro IrisLabel 16u 142u 185u 15u "$(IrisAnalyze)" "EDEDED" $IrisBoldFont
  !insertmacro IrisLabel 16u 158u 185u 14u "$(IrisAnalyzeBody)" "BDBDBD" $IrisSmallFont
  !insertmacro IrisLabel 16u 181u 290u 11u "$(IrisScope)" "BDBDBD" $IrisSmallFont

  ; An abstract contact sheet, not stock or user photographs.
  ${NSD_CreateLabel} 210u 77u 97u 83u ""
  Pop $0
  SetCtlColors $0 "EDEDED" "242424"
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
  SetCtlColors $0 "EDEDED" "242424"
  SendMessage $0 ${WM_SETFONT} $IrisSmallFont 1
  ${NSD_CreateLabel} 217u 148u 82u 10u "$(IrisPreviewNote)"
  Pop $0
  SetCtlColors $0 "BDBDBD" "242424"
  SendMessage $0 ${WM_SETFONT} $IrisSmallFont 1
  IrisTheme::Apply "welcome"
FunctionEnd

!macro IrisMove CONTROL X Y W H
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  IntOp $2 ${X} + ${W}
  IntOp $3 ${Y} + ${H}
  System::Call '*(i ${X}, i ${Y}, i r2, i r3) p.r0'
  System::Call 'user32::GetParent(p ${CONTROL}) p.r1'
  System::Call 'user32::MapDialogRect(p r1, p r0)'
  System::Call '*$0(i.r1, i.r2, i.r3, i.r4)'
  System::Free $0
  IntOp $3 $3 - $1
  IntOp $4 $4 - $2
  System::Call 'user32::MoveWindow(p ${CONTROL}, i r1, i r2, i r3, i r4, i 1)'
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

Function IrisFinishShow
  Call IrisFonts
  IrisTheme::Apply "finish"
  ; A required reboot remains an actionable system state, not decorative copy.
  ${If} ${RebootFlag}
    Return
  ${EndIf}
  GetDlgItem $0 $HWNDPARENT 2
  ShowWindow $0 ${SW_HIDE}
  GetDlgItem $0 $HWNDPARENT 3
  ShowWindow $0 ${SW_HIDE}
  ShowWindow $mui.FinishPage.Image ${SW_HIDE}
  ShowWindow $mui.FinishPage.Text ${SW_HIDE}
  SendMessage $mui.FinishPage.Title ${WM_SETFONT} $IrisHeadingFont 1
  !insertmacro IrisMove $mui.FinishPage.Title 16 18 290 28
  !insertmacro IrisMove $mui.FinishPage.Run 16 60 95 16
  !insertmacro IrisMove $mui.FinishPage.ShowReadme 122 60 183 16
FunctionEnd

Function IrisGuiInit
  SetCtlColors $HWNDPARENT "EDEDED" "242424"
  IrisTheme::Apply "base"
FunctionEnd
Function un.IrisGuiInit
  SetCtlColors $HWNDPARENT "EDEDED" "242424"
  IrisTheme::Apply "base"
FunctionEnd
Function IrisDirectoryShow
  IrisTheme::Apply "directory"
FunctionEnd
Function IrisProgressShow
  IrisTheme::Apply "progress"
FunctionEnd
Function un.IrisProgressShow
  IrisTheme::Apply "progress"
FunctionEnd
