#pragma once

#include <d3d9.h>
#include <vector>
#include <map>
#include <functional>

#include "Log.h"
#include "Input.h"
#include "Interop.h"

class Hook_IDirect3DVertexBuffer9;

namespace ModelMod {

#define MM_MAX_STAGE 16

struct D3DRenderState {
	struct StreamData {
		IDirect3DVertexBuffer9* pStreamData;
		UINT OffsetInBytes;
		UINT Stride;
		UINT StreamFreqSetting;
	};
	bool saved;

	DWORD V_FVF;
	StreamData Streams[1];
	IDirect3DVertexDeclaration9* V_Decl;
	IDirect3DIndexBuffer9* pIndexData;
	IDirect3DBaseTexture9* texture[MaxModTextures];
	IDirect3DVertexShader9* vertexShader;
	IDirect3DPixelShader9* pixelShader;

	DWORD CullMode;
	DWORD LightingEnabled;
	DWORD AlphaBlendEnabled;
	DWORD SamplerState0U;
	DWORD SamplerState0V;
	D3DMATRIX TexTransform0;
	D3DMATRIX World0;
	DWORD texture1ColoropState;

	D3DRenderState() {
		V_Decl = NULL;
		pIndexData = NULL;
		for (Uint32 i = 0; i < MaxModTextures; ++i)
			texture[i] = NULL;
		Streams[0].pStreamData = NULL;
		vertexShader = NULL;
		pixelShader = NULL;

		reset();
	}

	void reset() {
		SAFE_RELEASE(V_Decl);
		SAFE_RELEASE(pIndexData);
		for (Uint32 i = 0; i < MaxModTextures; ++i)
			SAFE_RELEASE(texture[i]);
		SAFE_RELEASE(Streams[0].pStreamData);
		SAFE_RELEASE(vertexShader);
		SAFE_RELEASE(pixelShader);
		memset(this, 0, sizeof(D3DRenderState));
	}
};

typedef map<UINT,ConstantData<float,4>> FloatConstantMap;
typedef map<UINT,ConstantData<int,4>> IntConstantMap;
typedef map<UINT,ConstantData<BOOL,1>> BoolConstantMap;
typedef std::map<int, std::function<void()>> InputKeyMap;

class RenderState : public ID3DResourceTracker, public IRenderState {
	static RenderState* _sCurrentRenderState;
	static const string LogCategory;

	InputKeyMap _punctKeyMap;
	InputKeyMap _fKeyMap;
	InputKeyMap* _pCurrentKeyMap;

	std::vector<void*> _textureHandles;
	std::map<void*,bool> _activeTextureLookup;
	std::vector<void*> _activeTextureList;
	std::vector<ISceneNotify*> _sceneNotify;

	int _currentTextureIdx;
	void* _currentTexturePtr;
	bool _selectedOnStage[MM_MAX_STAGE];
	bool _stageEnabled[MM_MAX_STAGE];

	ULONGLONG _snapStart;
	bool _snapRequested;
	bool _doingSnap;
	bool _initted;
	D3DLIGHT9 _lightZero;
	bool _hasLightZero;
	bool _showModMesh;
	bool _dipActive;
	
	Input _input;

	std::map<IUnknown*,bool> _d3dResources;

	typedef std::map<int,NativeModData> ManagedModMap;
	ManagedModMap _managedMods;

	HWND _focusWindow;
	IDirect3DDevice9* _dev;
	IDirect3DTexture9* _selectionTexture;
public:
	// Public data members
	// TODO: make these private with accessors if I decide to keep them

	D3DRenderState _d3dRenderState;

	FloatConstantMap vsFloatConstants;
	IntConstantMap vsIntConstants;
	BoolConstantMap vsBoolConstants;

	FloatConstantMap psFloatConstants;
	IntConstantMap psIntConstants;
	BoolConstantMap psBoolConstants;

	DWORD _lastFVF;
	bool _lastWasFVF;
	IDirect3DVertexDeclaration9* _lastDecl;

	Hook_IDirect3DVertexBuffer9* _currHookVB0; // track hook vb only for stream 0

public:
	RenderState(void);

	virtual ~RenderState(void);

	static bool exists() {
		return _sCurrentRenderState != NULL;
	}
	static RenderState& get() {
		return *_sCurrentRenderState;
	}

	typedef std::map<DWORD,std::string> TextureInfoMap;
	typedef std::map<DWORD,DWORD> StageStateMap;

	StageStateMap stageMap[MM_MAX_STAGE];

	TextureInfoMap texInfo;
	void shutdown();

	// Load (or reload) the managed code and MM registry configuration.  This also clears
	// the loaded mods, but doesn't reload them.
	void loadManagedAssembly();
	// load the mods 
	void loadMods(); 
	void loadEverything(); 

	void clearLoadedMods();

	NativeModData* findMod(int vertCount, int primCount);

	void init(IDirect3DDevice9* dev);

	void addSceneNotify(ISceneNotify* notify);

	void beginScene(IDirect3DDevice9* dev);

	void endScene(IDirect3DDevice9* dev);

	StageStateMap& getStageMap(DWORD Stage) { return stageMap[Stage]; }

	IDirect3DDevice9* getDevice() {
		return _dev;
	}
	IDirect3DTexture9* getSelectionTexture() { 
		return _selectionTexture;
	}
	bool isDIPActive() {
		return _dipActive;
	}
	void setDIPActive(bool active) {
		_dipActive = active;
	}

	void toggleShowModMesh() {
		_showModMesh = !_showModMesh;
	}

	bool getShowModMesh() {
		return _showModMesh;
	}

	void selectNextTexture();
	void selectPrevTexture();

	void setKeyMap();

	int currentTextureIdx() {
		return _currentTextureIdx;
	}

	IDirect3DBaseTexture9* currentTexture() {
		return (IDirect3DBaseTexture9*)_currentTexturePtr;
	}

	long selectedTextureStage() {
		// return true if any enabled stage has the selected texture
		for (Uint8 i = 0; i < MM_MAX_STAGE; ++i) {
			if (!_stageEnabled[i]) {
				return -1;
			}
			if (_selectedOnStage[i]) {
				return i;
			}
		}
		return -1;
	}

	void clearTextureLists() {
		_currentTextureIdx = -1;
		_currentTexturePtr = NULL;
		_activeTextureList.clear();
		_activeTextureLookup.clear();
	}

	void requestSnap() {
		MM_LOG_INFO(format("Snap is requested"));
		_snapRequested = true;
	}
	bool isSnapRequested() {
		return _snapRequested;
	}

	bool isDoingSnap() {
		return _doingSnap;
	}
	void startSnap() {
		_doingSnap = true;
		_snapStart = GetTickCount64();
	}
	void endSnap() {
		MM_LOG_INFO(format("ending snap"));
		_snapRequested = false;
		_doingSnap = false;
	}

	bool isSnapping() {
		return _snapRequested && _doingSnap && selectedTextureStage() >= 0;
	}

	void saveTexture(int index, WCHAR* path);

	// ---------------------------------------
	// ID3DResourceTracker


	void add(IUnknown* resource);

	void release(IUnknown* resource);

	// ---------------------------------------
	// IRenderState
	virtual void saveRenderState(IDirect3DDevice9* dev);

	virtual void restoreRenderState(IDirect3DDevice9* dev);

	// ---------------------------------------
	void textureCreated(IDirect3DTexture9* tex);

	void textureDeleted();

	void setTexture(DWORD Stage,IDirect3DBaseTexture9* pTexture);
	void setTextureStageState(DWORD Stage,D3DTEXTURESTAGESTATETYPE Type, DWORD Value);

	void setLight(DWORD Index,CONST D3DLIGHT9* light);
	void getLight(DWORD Index,D3DLIGHT9** light);
};

};