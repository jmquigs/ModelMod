#pragma once

#ifdef MMINTEROP_EXPORTS
#define MMINTEROP_API __declspec(dllexport)
#else
#define MMINTEROP_API __declspec(dllimport)
#endif

MMINTEROP_API int GetMMVersion();

extern "C" {

struct IDirect3D9;
struct IDirect3DDevice9;
struct IDirect3DVertexBuffer9;
struct IDirect3DIndexBuffer9;
struct IDirect3DVertexDeclaration9;
struct IDirect3DIndexBuffer9;

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
	char InputProfile[512];

	ConfData() {
		memset(this, 0, sizeof(ConfData));
	}
};
#pragma pack(pop)

typedef int (__stdcall *InitCallback) (int);
typedef ConfData* (__stdcall *SetPathsCB) (WCHAR*, WCHAR*);
typedef WCHAR* (__stdcall *GetDataPathCB) ();
typedef int (__stdcall *LoadModDBCB) ();
typedef int (__stdcall *GetModCountCB) ();
typedef ModData* (__stdcall *GetModDataCB) (int modIndex);
typedef int (__stdcall *FillModDataCB) (int modIndex, char* declData, int declSize, char* vbData, int vbSize, char* ibData, int ibSize);
typedef int (__stdcall *TakeSnapshotCB) (IDirect3DDevice9* device, SnapshotData* snapdata);

#pragma pack(push,8)
typedef struct {
	SetPathsCB SetPaths;
	GetDataPathCB GetDataPath;
	LoadModDBCB LoadModDB;
	GetModCountCB GetModCount;
	GetModDataCB GetModData;
	FillModDataCB FillModData;
	TakeSnapshotCB TakeSnapshot;
} ManagedCallbacks;
#pragma pack(pop)

MMINTEROP_API int OnInitialized(ManagedCallbacks* callbacks);
MMINTEROP_API void LogInfo(char* category, char* message);
MMINTEROP_API void LogWarn(char* category, char* message);
MMINTEROP_API void LogError(char* category, char* message);
MMINTEROP_API void SaveTexture(int index, WCHAR* path);

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

	NativeModData() {
		memset(this,0,sizeof(NativeModData));
	}

	static int hashCode(int vertCount, int primCount) {
		//https://en.wikipedia.org/wiki/Pairing_function#Cantor_pairing_function
		return ( (vertCount + primCount) * (vertCount + primCount + 1) / 2 ) + primCount;
	}
};

// Interal functions
namespace Interop {
	int InitCLR(WCHAR* mmPath);
	int ReloadAssembly();
	// If this returns false, calling a callback will explode in your face.
	bool OK(); 
	const ManagedCallbacks& Callbacks();
	const ConfData& Conf();
};