// MMLoader.cpp : Executable Loading and DLL Injection
//
// Borrows significantly from Oblivion Script Extender Source 
// http://obse.silverlock.org/
//

#include "stdafx.h"
#include "MMLoader.h"
#include "Inject.h"

#include <tlhelp32.h>
#include <shlwapi.h>

#include <stdio.h>
#include <string.h>

#include "Util.h"

// Global Variables:
HINSTANCE hInst;								// current instance
BOOL isFirstSearch = TRUE;

BOOL				InitInstance(HINSTANCE, int);
LRESULT CALLBACK	WndProc(HWND, UINT, WPARAM, LPARAM);

BOOL IterateProcessThreads( DWORD dwOwnerPID, bool suspend ) 
{ 
   HANDLE hThreadSnap = INVALID_HANDLE_VALUE; 
   THREADENTRY32 te32; 

   hThreadSnap = CreateToolhelp32Snapshot( TH32CS_SNAPTHREAD, 0 ); 
   if( hThreadSnap == INVALID_HANDLE_VALUE ) 
      return( FALSE ); 

   te32.dwSize = sizeof(THREADENTRY32); 

   if( !Thread32First( hThreadSnap, &te32 ) ) 
   {
      printf("Failed to open first thread\n");
      CloseHandle( hThreadSnap );
      return( FALSE );
   }

   // Walk the threads and suspend or resume as indicated; filter out threads
   // not belonging to target process.
   do 
   { 
      if( te32.th32OwnerProcessID == dwOwnerPID )
      {
		  HANDLE tHandle = OpenThread(THREAD_SUSPEND_RESUME,
			  FALSE,
			  te32.th32ThreadID);

		  if (tHandle)
		  {
			  int suspendCount = -1;
			  if (suspend) {
				  suspendCount = SuspendThread(tHandle);
				  printf("Suspend thread: %d\n", suspendCount);
			  }
			  else {
				  suspendCount = ResumeThread(tHandle);
				  printf("Resume thread: %d\n", suspendCount);
			  }

			  CloseHandle(tHandle);
		  }        
      }
   } while( Thread32Next(hThreadSnap, &te32 ) ); 

   CloseHandle( hThreadSnap );
   return( TRUE );
}

// Look for a process, then return the process ID (to be later used with IterateProcessThreads).
// If multiple processes match the name, the first match is used (i.e. this doesn't work well with multiprocess programs).
// Use a LONGLONG so that we can indicate a find error with -1.
LONGLONG FindProcess(string processName) {
	if (processName.empty()) {
		return -1;
	}

	HANDLE hSnapshot;
	PROCESSENTRY32 pe32;
	pe32.dwSize = sizeof(PROCESSENTRY32);

	// this will need to change if I want to support 64 bit.
	hSnapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
	if (hSnapshot == INVALID_HANDLE_VALUE)
	{
		printf("Failed to snapshot processes\n");
		return -1;
	}

	if (!Process32First(hSnapshot, &pe32))
	{
		printf("Failed to get information about first process\n");
		CloseHandle(hSnapshot);
		return -1;
	}


	Util::StrLower(processName);
	processName = Util::Basename(processName);

	DWORD foundID = 0;
	do
	{
		string spName = Util::Basename(pe32.szExeFile);

		Util::StrLower(spName);

		if (processName != spName) {
			continue;
		}

		printf("Found %s\n", processName.c_str());
		foundID = pe32.th32ProcessID;
		break;
	} while (Process32Next(hSnapshot, &pe32));

	CloseHandle(hSnapshot);

	return foundID;
}

LONGLONG FindAndSuspend(string processName) {
	LONGLONG foundID = FindProcess(processName);
	if (foundID > 0) {
		// suspend
		IterateProcessThreads((DWORD)foundID, true);
	}

	return foundID;
}

void ShowError(const string& doh) {
	printf("%s\n", doh.c_str());
	Util::DisplayMessageBox(doh.c_str(), "Crap");
}

int StartInjection(bool launch, string processName, string dllPath, int waitPeriod) {
	DWORD targetProcessId = 0;

	// Launch target EXE
	STARTUPINFO			startupInfo = { 0 };
	PROCESS_INFORMATION	procInfo = { 0 };

	if (launch) {
		printf("Launching %s\n", processName.c_str());

		bool result = CreateProcess(
			processName.c_str(),
			NULL,	// no args
			NULL,	// default process security
			NULL,	// default thread security
			TRUE,	// don't inherit handles
			CREATE_SUSPENDED,
			NULL,	// no new environment
			NULL,	// no new cwd
			&startupInfo, &procInfo) != 0;

		// check for Vista failing to create the process due to elevation requirements
		if(!result && (GetLastError() == ERROR_ELEVATION_REQUIRED))
		{
			Util::DisplayMessageBox("Elevation required, run as administrator",
				"Fail",
				MB_OK);
			return -1;
		}
	
		if (!result) {
			char err[65536];
			sprintf_s(err,sizeof(err), "Failed to Launch: %08X", GetLastError());
			Util::DisplayMessageBox(err,"Fail");
			return -1;
		}

		targetProcessId = procInfo.dwProcessId;
	} else {
		// continually watch for processes whose name matches processName.
		// when one is found, suspend all of its threads immediately.
		// requirement: target process must be started AFTER this process.  otherwise, we don't know 
		// how long its been running, so it isn't safe to inject (it may have already created its d3d device, etc).
		
		printf("Waiting for process: %s\n", processName.c_str());
		if (waitPeriod != -1) {
			printf("Wait period: %d seconds\n", waitPeriod);
		}
		else {
			printf("Waiting indefinitely\n");
		}

		// already running?
		LONGLONG foundID = FindProcess(processName);
		string findProcessErrorMessage = "Error attempting to find processes for name: " + string(processName);

		if (foundID == -1) {
			ShowError(findProcessErrorMessage);
			return -1;
		}
		if (isFirstSearch && foundID > 0) {
			string doh = "Found process on first search, aborting because it must be started after this process.\n";
			ShowError(doh);
			return -1;
		}
		isFirstSearch = FALSE;
		
		DWORD startTime = GetTickCount();

		// enter find and suspend loop
		do {
			Sleep(1);
			foundID = FindAndSuspend(processName);
			if (foundID == -1) {
				ShowError(findProcessErrorMessage);
				return -1;
			}
			targetProcessId = (DWORD)foundID;

			if (waitPeriod != -1 && targetProcessId == 0) {
				// check for timed exit
				DWORD elapsed = GetTickCount() - startTime;
				unsigned int waitMax = waitPeriod * 1000;
				if (elapsed >= waitMax) {
					printf("Wait period expired, exiting\n");
					return -1;
				}
			}
		} while(targetProcessId == 0);
	}

	if (targetProcessId == 0) {
		printf("No target process, unable to load\n");
		return -1;
	}

	// Inject
	Inject i;

	printf("Injecting %s\n", dllPath.c_str());

	if (!i.InjectDLL(targetProcessId,dllPath.c_str(), launch)) {
		printf("Inject error: %s\n", i.GetError().c_str());

		printf("Resuming target process due to injection failure; restart target process manually to try again\n");
		// let it go, let it gooooooo
		IterateProcessThreads(targetProcessId, false);
		//TerminateThread(procInfo.hThread, 666);

		Util::DisplayMessageBox("Failed to inject DLL", "Crap");
		return -1;
	} else {
	}

	// if we did not launch, need to open target process Id and wait for it to exit
	if (!launch) {
		procInfo.hProcess = OpenProcess(SYNCHRONIZE, FALSE, targetProcessId);
	}

	// Resume and Wait
	printf("Waiting for exit\n");
	IterateProcessThreads(targetProcessId, false);
	
	int ret = 0;

	DWORD waitRet = WaitForSingleObject(procInfo.hProcess, INFINITE);
	switch(waitRet)  // g_options.m_threadTimeout
	{
	case WAIT_OBJECT_0:
		break;

	case WAIT_ABANDONED:
		break;

	case WAIT_TIMEOUT:
		break;

	case WAIT_FAILED:
		waitRet = GetLastError();
		printf("Failed to wait for process to exit\n");
		ret = -1;
		break;
	default:
		break;
	}

	// clean up
	CloseHandle(procInfo.hProcess);
	CloseHandle(procInfo.hThread);

	return ret;
}

#if 0
int APIENTRY _tWinMain(HINSTANCE hInstance,
                     HINSTANCE hPrevInstance,
                     LPTSTR    lpCmdLine,
                     int       nCmdShow)
{
	UNREFERENCED_PARAMETER(hPrevInstance);

#else
int _tmain(int argc, const char* argv[])
{
	string targetExe = "";
#endif
	int waitPeriod = -1;

	for (int i = 0; i < argc; ++i) {
		//printf("%s\n", argv[i]);

		string arg = string(argv[i]);
		if (arg == "-waitperiod") {
			if (i + 1 < argc) {
				sscanf_s(argv[i + 1], "%d", &waitPeriod);
			}
		}
		
		if (targetExe.empty() && arg[0] != '-') {
			targetExe = arg;
		}
	}
	if (targetExe.empty()) {
		Util::DisplayMessageBox("Command line missing argument: path to executable to inject", "Crap");
		return -1;
	}
	else {
		targetExe = argv[1];
	}

	char thisFilePath[8192];
	GetModuleFileName(NULL, thisFilePath, sizeof(thisFilePath));
	char* lastBS = strrchr(thisFilePath, '\\');
	if (lastBS != NULL) {
		*lastBS = 0;
	}

	char dllPath[8192];
	sprintf_s(dllPath, sizeof(dllPath), "%s\\%s", thisFilePath, "ModelMod.dll");

	bool launch = false;

	FILE* fp = NULL;
	fopen_s(&fp, dllPath, "r");
	if (!fp) {
		Util::DisplayMessageBox("dll cannot be opened");
	} 
	if (fp) {
		fclose(fp);
	}

	fp = NULL;
	fopen_s(&fp, targetExe.c_str(), "r");
	if (!fp) {
		Util::DisplayMessageBox("exe cannot be opened");
	} 
	if (fp) {
		fclose(fp);
	}

	int ret = 0;
	if (launch) {
		ret = StartInjection(launch, targetExe, dllPath, waitPeriod);
	} else {
		// poll mode.
		// in this mode, only one instance of loader must be running for the target process name
		string lpName(targetExe);
		lpName = Util::ReplaceString(lpName, " ", "_");
		lpName = Util::ReplaceString(lpName, "\\", "_");
		lpName = Util::ReplaceString(lpName, ":", "_");

		string mutieName = string("MMLoader_For_") + string(lpName);
		SetLastError(ERROR_SUCCESS);
		HANDLE mutie = CreateMutex(NULL, TRUE, mutieName.c_str());
		DWORD err = GetLastError();
		if (!mutie || err != ERROR_SUCCESS) {
			string err = "Unable to create new mutex: " + string(mutieName) + "; another MMLoader may already be running for the process";
			Util::DisplayMessageBox(err.c_str());
		}
		else {
			// poll forever
			do {
				ret = StartInjection(launch, targetExe, dllPath, waitPeriod);
			} while (ret == 0);
		}
		CloseHandle(mutie);
	}


	return ret;

	//MSG msg;
	//HACCEL hAccelTable;

	//hAccelTable = LoadAccelerators(hInstance, MAKEINTRESOURCE(IDC_MMLOADER));

	//// Main message loop:
	//while (GetMessage(&msg, NULL, 0, 0))
	//{
	//	if (!TranslateAccelerator(msg.hwnd, hAccelTable, &msg))
	//	{
	//		TranslateMessage(&msg);
	//		DispatchMessage(&msg);
	//	}
	//}

	//return (int) msg.wParam;
}

//
//  FUNCTION: WndProc(HWND, UINT, WPARAM, LPARAM)
//
//  PURPOSE:  Processes messages for the main window.
//
//  WM_COMMAND	- process the application menu
//  WM_PAINT	- Paint the main window
//  WM_DESTROY	- post a quit message and return
//
//
LRESULT CALLBACK WndProc(HWND hWnd, UINT message, WPARAM wParam, LPARAM lParam)
{
	int wmId, wmEvent;
	PAINTSTRUCT ps;
	HDC hdc;

	switch (message)
	{
	case WM_COMMAND:
		wmId    = LOWORD(wParam);
		wmEvent = HIWORD(wParam);
		// Parse the menu selections:
		switch (wmId)
		{
		case IDM_EXIT:
			DestroyWindow(hWnd);
			break;
		default:
			return DefWindowProc(hWnd, message, wParam, lParam);
		}
		break;
	case WM_PAINT:
		hdc = BeginPaint(hWnd, &ps);
		EndPaint(hWnd, &ps);
		break;
	case WM_DESTROY:
		PostQuitMessage(0);
		break;
	default:
		return DefWindowProc(hWnd, message, wParam, lParam);
	}
	return 0;
}
