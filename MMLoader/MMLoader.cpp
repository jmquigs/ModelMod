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
#include <shellapi.h>

#include <stdio.h>
#include <string.h>

#include "Util.h"

// Global Variables:
HINSTANCE hInst;								// current instance
BOOL isFirstSearch = TRUE;

BOOL				InitInstance(HINSTANCE, int);
LRESULT CALLBACK	WndProc(HWND, UINT, WPARAM, LPARAM);

BOOL SuspendProcessThreads( DWORD dwOwnerPID, bool suspend ) 
{ 
   HANDLE hThreadSnap = INVALID_HANDLE_VALUE; 
   THREADENTRY32 te32; 

   hThreadSnap = CreateToolhelp32Snapshot( TH32CS_SNAPTHREAD, 0 ); 
   if( hThreadSnap == INVALID_HANDLE_VALUE ) 
      return( FALSE ); 

   te32.dwSize = sizeof(THREADENTRY32); 

   if( !Thread32First( hThreadSnap, &te32 ) ) 
   {
      Util::Log("Failed to open first thread\n");
      CloseHandle( hThreadSnap );
      return( FALSE );
   }

   // Walk the threads and suspend or resume as indicated; filter out threads
   // not belonging to target process.
   int numThreads = 0;
   do 
   { 
      if( te32.th32OwnerProcessID == dwOwnerPID )
      {
		  numThreads++;
		  HANDLE tHandle = OpenThread(THREAD_SUSPEND_RESUME,
			  FALSE,
			  te32.th32ThreadID);

		  if (tHandle)
		  {
			  int suspendCount = -1;
			  if (suspend) {
				  suspendCount = SuspendThread(tHandle);
				  Util::Log("Suspend thread %08X: %d\n", te32.th32ThreadID, suspendCount);
			  }
			  else {
				  suspendCount = ResumeThread(tHandle);
				  Util::Log("Resume thread %08X: %d\n", te32.th32ThreadID, suspendCount);
			  }

			  CloseHandle(tHandle);
		  }        
      }
   } while( Thread32Next(hThreadSnap, &te32 ) ); 

   Util::Log("Processed %d threads\n", numThreads);
   CloseHandle( hThreadSnap );
   return( TRUE );
}

// Look for a process by name, then return the process ID (to be later used with SuspendProcessThreads).
// If multiple processes match the name, the first match is used (i.e. this doesn't work with multiprocess programs).
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
		Util::Log("Failed to snapshot processes\n");
		return -1;
	}

	if (!Process32First(hSnapshot, &pe32))
	{
		Util::Log("Failed to get information about first process\n");
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

		Util::Log("Found %s\n", processName.c_str());
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
		SuspendProcessThreads((DWORD)foundID, true);
	}

	return foundID;
}

void ShowError(const string& doh) {
	Util::Log("%s\n", doh.c_str());
	Util::DisplayMessageBox(doh.c_str(), "Crap");
}

int StartInjection(bool launch, string processName, string dllPath, int waitPeriod) {
	DWORD targetProcessId = 0;

	// Launch target EXE
	STARTUPINFO			startupInfo = { 0 };
	PROCESS_INFORMATION	procInfo = { 0 };

	if (launch) {
		Util::Log("Launching %s\n", processName.c_str());

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
		
		const char* cname = processName.c_str();
		Util::Log("Waiting for process: %s\n", cname);
		if (waitPeriod != -1) {
			Util::Log("Wait period: %d seconds\n", waitPeriod);
		}
		else {
			Util::Log("Waiting indefinitely\n");
		}

		// check to see if already running

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
		
		ULONGLONG startTime = GetTickCount64();

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
				ULONGLONG elapsed = GetTickCount64() - startTime;
				unsigned int waitMax = waitPeriod * 1000;
				if (elapsed >= waitMax) {
					Util::Log("Wait period expired, exiting\n");
					return -2;
				}
			}
		} while(targetProcessId == 0);
	}

	if (targetProcessId == 0) {
		Util::Log("No target process, unable to load\n");
		return -1;
	}

	Inject i;
	Util::Log("Injecting %s\n", dllPath.c_str());

	if (!i.InjectDLL(targetProcessId,dllPath.c_str(), launch)) {
		Util::Log("Inject error: %s\n", i.GetError().c_str());

		// could terminate, but we don't want to leave it in a bad state, so 
		// let it go, let it gooooooo
		Util::Log("Resuming target process due to injection failure; restart target process manually to try again\n");
		
		SuspendProcessThreads(targetProcessId, false);

		Util::DisplayMessageBox("Failed to inject DLL", "Crap");
		return -1;
	} 

	// if we did not launch, need to open target process Id and wait for it to exit
	if (!launch) {
		procInfo.hProcess = OpenProcess(SYNCHRONIZE, FALSE, targetProcessId);
	}

	// Resume and Wait
	Util::Log("Waiting for exit\n");
	SuspendProcessThreads(targetProcessId, false);
	
	int ret = 0;

	DWORD waitRet = WaitForSingleObject(procInfo.hProcess, INFINITE);
	switch(waitRet)  
	{
	case WAIT_OBJECT_0:
		Util::Log("Got WAIT_OBJECT_0\n");
		break;

	case WAIT_ABANDONED:
		Util::Log("Got WAIT_ABANDONED\n");
		break;

	case WAIT_TIMEOUT:
		Util::Log("Got WAIT_TIMEOUT\n");
		break;

	case WAIT_FAILED:
		waitRet = GetLastError();
		Util::Log("Failed to wait for process to exit\n");
		ret = -1;
		break;
	default:
		Util::Log("Got WaitForSingleObject code %08X", waitRet);
		break;
	}

	// clean up
	CloseHandle(procInfo.hProcess);
	CloseHandle(procInfo.hThread);

	return ret;
}

class LogExitTime {
public:
	LogExitTime() {

	}

	virtual ~LogExitTime() {
		SYSTEMTIME lt;
		GetLocalTime(&lt);
		Util::Log("Loader exiting at: %d/%d/%d %02d:%02d:%02d\n", lt.wMonth, lt.wDay, lt.wYear, lt.wHour, lt.wMinute, lt.wSecond);
	}
};

// toggle this to building as a windows or console app (to see printf, etc)
//#define BUILD_CONSOLE

#ifndef BUILD_CONSOLE
int WINAPI _tWinMain(
	_In_ HINSTANCE hInstance,
	_In_opt_ HINSTANCE hPrevInstance,
	_In_ LPSTR lpCmdLine,
	_In_ int nShowCmd
	)
{
	UNREFERENCED_PARAMETER(hPrevInstance);

	LPWSTR* uArgv;
	int argc;

	uArgv = CommandLineToArgvW(GetCommandLineW(), &argc);
	if (uArgv == NULL) {
		Util::DisplayMessageBox("Failed to parse command line args", "Crap");
		return -1;
	}

	// convert to multi-byte until I do the real fix and convert the rest of this program to wide.
	// the memory associated with these args is leaked...ZOMG
	char** argv = new char*[argc]; 
	if (argc > 0) {
		for (int i = 0; i < argc; ++i) {
			argv[i] = Util::ConvertToMB(uArgv[i]);
		}
	}
#else
int _tmain(int argc, const char* argv[])
{
#endif
	string targetExe = "";
	int waitPeriod = -1;
	string logFile = "";

	// process command line
	//Util::Log("Args:\n");
	for (int i = 1; i < argc; ++i) {
		//Util::Log("  %s\n", argv[i]);
		string arg = string(argv[i]);
		if (arg == "-waitperiod") {
			if (i + 1 < argc) {
				sscanf_s(argv[i + 1], "%d", &waitPeriod);
			}
		}
		if (arg == "-logfile") {
			if (i + 1 < argc) {
				logFile = string(argv[i + 1]);
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

	// we always use poll mode rather than launch, this is just here for debug launches
	bool launch = false;

	// if in poll mode, check mutex
	HANDLE mutie = NULL;
	if (!launch) {
		// in this mode, only one instance of loader must be running for the target process name.
		// create a mutex to enforce this.
		string lpName(targetExe);
		lpName = Util::ReplaceString(lpName, " ", "_");
		lpName = Util::ReplaceString(lpName, "\\", "_");
		lpName = Util::ReplaceString(lpName, ":", "_");

		string mutieName = string("MMLoader_For_") + string(lpName);
		SetLastError(ERROR_SUCCESS);
		mutie = CreateMutex(NULL, TRUE, mutieName.c_str()); 
		DWORD err = GetLastError();
		if (!mutie || err != ERROR_SUCCESS) {
			string err = "Unable to create new mutex: " + string(mutieName) + "; another MMLoader may already be running for the process";
			Util::Log("Error: %s\n", err.c_str());
			return -5;
		}
	}

	if (!logFile.empty()) {
		// fail if we can't open it; we're too stupid to try to create directories and stuff.
		// but make sure they didn't screw up the parameters
		string loglwr(logFile);
		Util::StrLower(loglwr);
		if (Util::HasEnding(loglwr, string(".exe"))) {
			Util::DisplayMessageBox("Yo! An exe is specified as the log file, yo.", "Crap");
			return -1;
		}
		FILE* lfp = NULL;
		DeleteFile(logFile.c_str());
		fopen_s(&lfp, logFile.c_str(), "wc");
		if (lfp == NULL) {
			Util::DisplayMessageBox("Failed to open output log file; does directory exist?", "Crap");
			return -1;
		}
		Util::SetLogFile(lfp);
	}

	SYSTEMTIME lt;
	GetLocalTime(&lt);
	Util::Log("Loader launched at: %d/%d/%d %02d:%02d:%02d\n", lt.wMonth, lt.wDay, lt.wYear, lt.wHour, lt.wMinute, lt.wSecond);
	LogExitTime foo;

	char thisFilePath[8192];
	GetModuleFileName(NULL, thisFilePath, sizeof(thisFilePath));
	char* lastBS = strrchr(thisFilePath, '\\');
	if (lastBS != NULL) {
		*lastBS = 0;
	}

	char dllPath[8192];
	sprintf_s(dllPath, sizeof(dllPath), "%s\\%s", thisFilePath, "ModelMod.dll");

	FILE* fp = NULL;
	fopen_s(&fp, dllPath, "r");
	if (!fp) {
		Util::DisplayMessageBox("dll cannot be opened");
		return -1;
	} 
	if (fp) {
		fclose(fp);
	}

	fp = NULL;
	fopen_s(&fp, targetExe.c_str(), "r");
	if (!fp) {
		Util::DisplayMessageBox("exe cannot be opened");
		return -1;
	} 
	if (fp) {
		fclose(fp);
	}

	int ret = 0;

	int attemptedInjections = 0;
	int successfulInjections = 0;

	if (launch) {
		ret = StartInjection(launch, targetExe, dllPath, waitPeriod);
	} else {
#ifndef BUILD_CONSOLE
		// need to pump message loop as windows app so that we are not "Not Responding"
		MSG msg;
		HACCEL hAccelTable;

		hAccelTable = LoadAccelerators(hInstance, MAKEINTRESOURCE(IDC_MMLOADER));
#endif
		// poll mode.
			// poll forever or until the wait period expires (StartInjection will return nonzero)
		do {
#ifndef BUILD_CONSOLE
			while (PeekMessage(&msg, NULL, 0, 0, PM_REMOVE))
			{
				if (!TranslateAccelerator(msg.hwnd, hAccelTable, &msg))
				{
					TranslateMessage(&msg);
					DispatchMessage(&msg);
				}
			}
#endif
			ret = StartInjection(launch, targetExe, dllPath, waitPeriod);

			if (ret == 0) {
				attemptedInjections++;
				successfulInjections++;
			}
			else if (ret == -1) {
				attemptedInjections++;
			}

			if (ret == -2) {
				// wait period expired; rewrite return code so that it indicates whether we successfully 
				// injected on each attempt, or there were some failures

				if (attemptedInjections == 0) {
					// did not attempt anything
					ret = -3;
				}
				else if (attemptedInjections == successfulInjections) {
					// all attempts were successful
					ret = 0;
					break; // bust out of loop
				}
				else {
					// some errors
					ret = -4;
				}

			}
		} while (ret == 0);
	}

	if (mutie != NULL) {
		CloseHandle(mutie);
	}

	return ret;
}

#ifndef BUILD_CONSOLE
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
#endif