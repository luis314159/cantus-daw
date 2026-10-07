; Give a shortcut an AppUserModelID (System.AppUserModel.ID), with the System plug-in's
; COM calls only: IShellLinkW -> IPersistFile::Load, IPropertyStore::SetValue + Commit,
; IPersistFile::Save. The program it starts sets the same ID for its process, so the
; taskbar groups its windows under the shortcut (and pins the shortcut, not the program).
; $R5 = 0 on success, else the failing HRESULT.
!macro CANTUS_SET_APP_ID LNK ID
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6
  StrCpy $R5 1
  System::Call 'ole32::CoCreateInstance(g "{00021401-0000-0000-C000-000000000046}", p 0, i 1, g "{000214F9-0000-0000-C000-000000000046}", *p .r1) i .r0'
  ${If} $0 = 0
    System::Call '$1->0(g "{0000010b-0000-0000-C000-000000000046}", *p .r2) i .r0'
    ${If} $0 = 0
      System::Call '$2->5(w "${LNK}", i 2) i .r0'
      ${If} $0 = 0
        System::Call '$1->0(g "{886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99}", *p .r3) i .r0'
        ${If} $0 = 0
          ; PROPERTYKEY: {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}, 5
          System::Call '*(g "{9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}", i 5) p .r4'
          ; PROPVARIANT: VT_LPWSTR (31), the string from SHStrDupW (CoTaskMem).
          System::Call 'shlwapi::SHStrDupW(w "${ID}", *p .r6) i .r0'
          System::Call '*(&i2 31, &i2 0, &i2 0, &i2 0, p r6, i 0) p .r5'
          System::Call '$3->6(p r4, p r5) i .r0'
          ${If} $0 = 0
            System::Call '$3->7() i .r0'
            ${If} $0 = 0
              System::Call '$2->6(p 0, i 1) i .r0'
            ${EndIf}
          ${EndIf}
          StrCpy $R5 $0
          System::Call 'ole32::CoTaskMemFree(p r6)'
          System::Free $5
          System::Free $4
          System::Call '$3->2()'
        ${EndIf}
      ${EndIf}
      System::Call '$2->2()'
    ${EndIf}
    System::Call '$1->2()'
  ${EndIf}
  ${If} $R5 = 1
    StrCpy $R5 $0
  ${EndIf}
  Pop $6
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
!macroend
