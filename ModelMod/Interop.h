// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

#pragma once

#ifdef INTEROP_EXPORTS
#define INTEROP_API __declspec(dllexport)
#else
#define INTEROP_API __declspec(dllimport)
#endif

INTEROP_API int GetMMVersion();

extern "C" {

struct IDirect3D9;
struct IDirect3DDevice9;
struct IDirect3DVertexBuffer9;
struct IDirect3DIndexBuffer9;
struct IDirect3DVertexDeclaration9;
struct IDirect3DIndexBuffer9;
struct IDirect3DBaseTexture9;

#define MaxModTextures 4
#define MaxModTexPathLen 8192 // Must match SizeConst attribute in managed code
typedef WCHAR ModTexPath[MaxModTexPathLen];

#pragma pack(push,8)
struct ModData {
	int modType;
	int primType;
	int vertCount;
	int primCount;
	int indexCount;
	int refVertCount;
	int refPrimCount;
	int declSizeBytes;
	int vertSizeBytes;
	int indexElemSizeBytes;
	ModTexPath texPath[MaxModTextures];

	ModData() {
		memset(this, 0, sizeof(ModData));
	}
};
#pragma pack(pop)

#pragma pack(push,8)
struct SnapshotData {
	int primType;
	int baseVertexIndex;
	unsigned int minVertexIndex;
	unsigned int numVertices;
	unsigned int startIndex;
	unsigned int primCount;

	IDirect3DVertexDeclaration9* decl;
	IDirect3DIndexBuffer9* ib;

	SnapshotData() {
		memset(this,0,sizeof(SnapshotData));
	}
};
#pragma pack(pop)

#pragma pack(push,8)
struct ConfData {
	// Note: marshalling to bool requires [<MarshalAs(UnmanagedType.I1)>] on the field in managed code; otherwise it will try to marshall it as a 4 byte BOOL,
	// which has a detrimental effect on subsequent string fields!
	bool RunModeFull;
	bool LoadModsOnStart;
	char InputProfile[512];

	ConfData() {
		memset(this, 0, sizeof(ConfData));
	}
};
#pragma pack(pop)

typedef int (__stdcall *InitCallback) (int);
typedef ConfData* (__stdcall *SetPathsCB) (WCHAR*, WCHAR*);
typedef int (__stdcall *LoadModDBCB) ();
typedef int (__stdcall *GetModCountCB) ();
typedef ModData* (__stdcall *GetModDataCB) (int modIndex);
typedef int (__stdcall *FillModDataCB) (int modIndex, char* declData, int declSize, char* vbData, int vbSize, char* ibData, int ibSize);
typedef int (__stdcall *TakeSnapshotCB) (IDirect3DDevice9* device, SnapshotData* snapdata);

#pragma pack(push,8)
typedef struct {
	SetPathsCB SetPaths;
	LoadModDBCB LoadModDB;
	GetModCountCB GetModCount;
	GetModDataCB GetModData;
	FillModDataCB FillModData;
	TakeSnapshotCB TakeSnapshot;
} ManagedCallbacks;
#pragma pack(pop)

INTEROP_API int OnInitialized(ManagedCallbacks* callbacks);
INTEROP_API void LogInfo(char* category, char* message);
INTEROP_API void LogWarn(char* category, char* message);
INTEROP_API void LogError(char* category, char* message);
INTEROP_API bool SaveTexture(int index, WCHAR* path);
//INTEROP_API bool SaveVertexShader(WCHAR* path);
INTEROP_API bool SavePixelShader(WCHAR* path);


};

// This has no representation in managed code.
struct NativeModData {
	ModData modData;
	char* vbData;
	char* ibData;
	char* declData;
	IDirect3DVertexBuffer9* vb;
	IDirect3DIndexBuffer9* ib;
	IDirect3DVertexDeclaration9* decl;
	IDirect3DBaseTexture9* texture[MaxModTextures];

	NativeModData() {
		memset(this,0,sizeof(NativeModData));
	}

	static int hashCode(int vertCount, int primCount) {
		//https://en.wikipedia.org/wiki/Pairing_function#Cantor_pairing_function
		return ( (vertCount + primCount) * (vertCount + primCount + 1) / 2 ) + primCount;
	}
};

// Interface used by the rest of the native code; all access to managed code must go through here.
namespace Interop {
	int InitCLR(WCHAR* mmPath);
	int ReloadAssembly();
	// If this returns false, calling a callback will explode in your face.
	bool OK(); 
	const ManagedCallbacks& Callbacks();
	const ConfData& Conf();
};