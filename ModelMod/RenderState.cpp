#include "RenderState.h"
using namespace ModelMod;

#include <d3dx9tex.h>

#include "Util.h"

#include "MMInterop.h"

RenderState* RenderState::_sCurrentRenderState = NULL;
const string RenderState::LogCategory = "RenderState";

namespace ModelMod {

RenderState::RenderState(void) :
		_currentTextureIdx(-1),
		_currentTexturePtr(NULL),
		_hasLightZero(false),
		_snapRequested(false),
		_doingSnap(false),
		_snapStart(0),
		_initted(false),
		_showModMesh(false),
		_dipActive(false),
		_dev(NULL),
		_focusWindow(INVALID_HANDLE_VALUE),
		_selectionTexture(NULL),
		_currHookVB0(NULL)
		//_modTexture(NULL),
		//_modVertexShader(NULL),
		//_modPixelShader(NULL) 
		{
		_sCurrentRenderState = this;

		memset(&_lightZero, 0, sizeof(D3DLIGHT9));
		memset(_selectedOnStage, 0, sizeof(bool) * MM_MAX_STAGE);
		memset(_stageEnabled, 0, sizeof(bool) * MM_MAX_STAGE);

		_lastFVF = 0;
		_lastWasFVF = false;
		_lastDecl = NULL;
	}

RenderState::~RenderState(void) {
		shutdown();
	}

void RenderState::shutdown() {
	MM_LOG_INFO("Releasing d3d resources");
	while (_d3dResources.size() > 0) {
		release(_d3dResources.begin()->first);
	}
}

// TODO, refactor if I move to VS2013 and get lambdas
static size_t findLastSlash(string str) {
	size_t slashIdx = str.find_last_of("\\");
	if (slashIdx == string::npos) {
		slashIdx = str.find_last_of("/");
	}
	return slashIdx;
}

void RenderState::loadMeshes() {
	for (ManagedModMap::iterator iter = _managedMods.begin();
		iter != _managedMods.end();
		++iter) {
			NativeModData& mod = iter->second;
			if (mod.vb) {
				release(mod.vb);
			}
			if (mod.ib) {
				release(mod.ib);
			}
			if (mod.decl) {
				release(mod.decl);
			}
	}
	_managedMods.clear();

	Interop::ReloadAssembly();

	DWORD start,elapsed;
	start = GetTickCount();
	if (Interop::OK()) {
		Interop::Callbacks().LoadModDB();

		int modCount = Interop::Callbacks().GetModCount();
		for (int i = 0; i < modCount; ++i) {
			ModData* mdat = Interop::Callbacks().GetModData(i);
			if (!mdat) {
				MM_LOG_INFO(format("Null mod data for index {}", i));
				continue;
			}
			if (mdat->modType != GPUReplacement && mdat->modType != Deletion) {
				MM_LOG_INFO(format("Unsupported type: {}", mdat->modType));
				continue;
			}

			NativeModData nModData;
			nModData.modData = *mdat;

			if (mdat->modType == Deletion) {
				int hashCode = NativeModData::hashCode(nModData.modData.refVertCount, nModData.modData.refPrimCount);
				_managedMods[hashCode] = nModData; // structwise-copy is ok
				// thats all we need to do for these.
				continue;
			}

			int declSize = mdat->declSizeBytes;
			char* declData = declSize > 0 ? new char[declSize] : NULL;

			int vbSize = mdat->primCount * 3 * mdat->vertSizeBytes;
			char* vbData = NULL;

			// index buffers not currently supported
			int ibSize = 0; //mdat->indexCount * mdat->indexElemSizeBytes;
			char* ibData = NULL;

			HRESULT hr;
			hr = _dev->CreateVertexBuffer(vbSize, D3DUSAGE_WRITEONLY, 0, D3DPOOL_MANAGED, &nModData.vb, NULL);
			if (FAILED(hr)) {
				MM_LOG_INFO("failed to create vertex buffer");
				return;
			}
			this->add(nModData.vb);

			hr = nModData.vb->Lock(0, 0, (void**)&vbData, 0);
			if (FAILED(hr)) {
				MM_LOG_INFO("failed to lock vertex buffer");
				return;

			}

			int ret = Interop::Callbacks().FillModData(i, declData, declSize, vbData, vbSize, ibData, ibSize);

			hr = nModData.vb->Unlock();
			if (FAILED(hr)) {
				MM_LOG_INFO("failed to unlock vb");
				return;
			}

			if (ret == 0) {
				// create vertex declaration
				_dev->CreateVertexDeclaration((D3DVERTEXELEMENT9*)declData, &nModData.decl);
				if (nModData.decl) {
					this->add(nModData.decl);
				}

				int hashCode = NativeModData::hashCode(nModData.modData.refVertCount, nModData.modData.refPrimCount);
				_managedMods[hashCode] = nModData; // structwise-copy is ok
			}

			delete [] declData; // TODO: ok to delete this, or does d3d decl reference it?
		}
	}
	elapsed = GetTickCount() - start;
	MM_LOG_INFO(format("Measured managed load time: {}", elapsed));
}

NativeModData* RenderState::findMod(int vertCount, int primCount) {
	int hashCode = NativeModData::hashCode(vertCount, primCount);
	if (_managedMods.count(hashCode)) {
		return &_managedMods[hashCode];
	}
	return NULL;
}

void RenderState::init(IDirect3DDevice9* dev) {
	_dev = dev;

	D3DDEVICE_CREATION_PARAMETERS params;
	HRESULT hr = dev->GetCreationParameters(&params);
	if (FAILED(hr)) {
		MM_LOG_INFO(format("Failed to obtain device creation parameters"));
	}
	else {
		_focusWindow = params.hFocusWindow;
	}


	// Usually, in dev mode, don't want to load on startup. Use the keybinding to load meshes.
	//loadMeshes();

	// load "selected" texture.  maybe should just generate this
	WCHAR* dataPath = NULL;
	if (Interop::OK() && (dataPath = Interop::Callbacks().GetDataPath()) != NULL) {
		IDirect3DTexture9 * tex;

		WCHAR fullpath[16384];
		swprintf_s(fullpath, sizeof(fullpath)/sizeof(fullpath[0]), L"%S/selected_texture.png", dataPath);

		HRESULT hr = D3DXCreateTextureFromFileW(dev, fullpath, &tex);
		if (FAILED(hr)) {
			MM_LOG_INFO("Failed to create 'selection' texture");
		} else {
			_selectionTexture = tex;
		}
	}

	_initted = true;
}

void RenderState::addSceneNotify(ISceneNotify* notify) {
	if (!notify)
		return;
	_sceneNotify.push_back(notify);
}

static const bool SnapWholeScene = false;
// Snapshotting is currently stops after a certain amount of real time has passed from the start of the snap, specified by this constant.
// One might expect that just snapping everything drawn within a single begin/end scene combo is sufficient, but this often misses data, 
// and sometimes fails to snapshot anything at all.  Perhaps the game is using multiple being/end combos, or drawing outside of begin/end 
// block.  Using a window makes it much more likely that something useful is captured, at the expense of some duplicates; even though
// some objects may still be missed.  Some investigation to make this more reliable would be useful.
static const ULONGLONG SnapMS = 250;

void RenderState::beginScene(IDirect3DDevice9* dev) {
	// TODO (renderstate) re-init when device changes (re-create resources)
	if (!_initted)
		init(dev);

	// process input only when the d3d window is in the foreground.  this style of processing creates issues for keyup processing, 
	// since we can lose events, but we don't do any of that currently.
	if (_focusWindow != INVALID_HANDLE_VALUE 
		//&& GetForegroundWindow() == _focusWindow // TODO: disable this, _focusWindow is actually not correct in some cases, so input is always disabled then
		) {
		vector<Input::KeyEvent> events = _input.update();

		for (Uint32 i = 0; i < events.size(); ++i) {
			Input::KeyEvent& evt = events[i];

			if (evt.pressed) {
				if (_input.isCtrlPressed()) {
					switch (evt.key) {
					case DIK_COMMA:
						selectNextTexture();
						break;
					case DIK_PERIOD:
						selectPrevTexture();
						break;
					case DIK_Z:
					case DIK_A:
						MM_LOG_INFO(format("Snap is requested"));
						requestSnap();
						break;
					case DIK_T:
						_currentTextureIdx = -1;
						_currentTexturePtr = NULL;
						_activeTextureList.clear();
						_activeTextureLookup.clear();
						break;
					case DIK_SEMICOLON:
						toggleShowModMesh();
						break;
					case DIK_SLASH:
						loadMeshes();
						break;
					}
				}
			}
			else {
				// No need for handling keyup currently
				//switch (evt.key) {
				//	case DIK_Q:
				//		break;
				//}
			}
		}
	}

	if (SnapWholeScene && currentTextureIdx() > 0) 
		requestSnap();

	if (!isDoingSnap() && isSnapRequested())
		startSnap();

	for (Uint32 i = 0; i < _sceneNotify.size(); ++i) {
		_sceneNotify[i]->onBeginScene();
	}
}

void RenderState::endScene(IDirect3DDevice9* dev) {
	dev;

	for (Uint32 i = 0; i < _sceneNotify.size(); ++i) {
		_sceneNotify[i]->onEndScene();
	}

	if (isDoingSnap() && (GetTickCount64() - _snapStart) > SnapMS) {
		endSnap();

		if (SnapWholeScene && currentTextureIdx() > 0)
			selectNextTexture();
	}
}

void RenderState::selectNextTexture() {
	if (_activeTextureList.size() == 0) {
		MM_LOG_INFO("No textures available");
		return;
	}

	_currentTextureIdx++;

	if ((unsigned int)_currentTextureIdx >= _activeTextureList.size())
		_currentTextureIdx = 0;

	_currentTexturePtr = _activeTextureList[_currentTextureIdx];

	MM_LOG_INFO(format("Current texture set to: {:x}", (int)_currentTexturePtr));
	//MM_LOG_INFO(texInfo[_currentTexturePtr]);
}

void RenderState::selectPrevTexture() {
	if (_activeTextureList.size() == 0) {
		MM_LOG_INFO("No textures available");
		return;
	}

	_currentTextureIdx--;

	if (_currentTextureIdx < 0)
		_currentTextureIdx = _activeTextureList.size() - 1;

	_currentTexturePtr = _activeTextureList[_currentTextureIdx];

	MM_LOG_INFO(format("Current texture set to: {:x}", (int)_currentTexturePtr));
	//MM_LOG_INFO(texInfo[_currentTexturePtr]);
}

// ---------------------------------------
// ID3DResourceTracker


void RenderState::add(IUnknown* resource) {
	if (_d3dResources.find(resource) != _d3dResources.end())
		return;
	_d3dResources[resource] = true;
}

void RenderState::release(IUnknown* resource) {
	if (_d3dResources.find(resource) == _d3dResources.end())
		return;
	resource->Release();
	_d3dResources.erase(resource);
}

// ---------------------------------------
// IRenderState
void RenderState::saveRenderState(IDirect3DDevice9* dev) {
	if (_d3dRenderState.saved) {
		MM_LOG_INFO("Existing D3D render state was not restored");
		return;
	}
	_d3dRenderState.reset();
	_d3dRenderState.saved = true;

	dev->GetFVF(&_d3dRenderState.V_FVF); // copy, no release
	dev->GetStreamSource(0,&(_d3dRenderState.Streams[0].pStreamData),&(_d3dRenderState.Streams[0].OffsetInBytes),&(_d3dRenderState.Streams[0].Stride)); // definitely release
	dev->GetStreamSourceFreq(0,&(_d3dRenderState.Streams[0].StreamFreqSetting)); // copy, no release
	dev->GetVertexDeclaration(&_d3dRenderState.V_Decl); // release not in docs?
	dev->GetIndices(&_d3dRenderState.pIndexData); // definitely release
	dev->GetRenderState(D3DRS_CULLMODE, &_d3dRenderState.CullMode); // copy, no release
	dev->GetSamplerState(0, D3DSAMP_ADDRESSU, &_d3dRenderState.SamplerState0U); // copy, no release
	dev->GetSamplerState(0, D3DSAMP_ADDRESSV, &_d3dRenderState.SamplerState0V); // copy, no release
	dev->GetTransform(D3DTS_TEXTURE0, &_d3dRenderState.TexTransform0); // copy, no release
	dev->GetTransform(D3DTS_WORLD, &_d3dRenderState.World0); // copy, no release
	dev->GetRenderState(D3DRS_LIGHTING, &_d3dRenderState.LightingEnabled); // copy, no release
	dev->GetRenderState(D3DRS_ALPHABLENDENABLE, &_d3dRenderState.AlphaBlendEnabled); // copy, no release
	dev->GetTexture(0, &_d3dRenderState.texture0); // definitely release
	dev->GetTexture(1, &_d3dRenderState.texture1); // definitely release
	dev->GetTextureStageState(1, D3DTSS_COLOROP, &_d3dRenderState.texture1ColoropState); // copy, no release
	dev->GetVertexShader(&_d3dRenderState.vertexShader); // release not in docs?
	dev->GetPixelShader(&_d3dRenderState.pixelShader); // release not in docs?
}

void RenderState::restoreRenderState(IDirect3DDevice9* dev) {
	if (!_d3dRenderState.saved) {
		MM_LOG_INFO("No D3D render state was saved, can't restore");
		return;
	}
	dev->SetFVF(_d3dRenderState.V_FVF);
	dev->SetStreamSource(0,(_d3dRenderState.Streams[0].pStreamData),(_d3dRenderState.Streams[0].OffsetInBytes),(_d3dRenderState.Streams[0].Stride));
	dev->SetStreamSourceFreq(0,(_d3dRenderState.Streams[0].StreamFreqSetting));
	dev->SetVertexDeclaration(_d3dRenderState.V_Decl);
	dev->SetIndices(_d3dRenderState.pIndexData);
	dev->SetRenderState(D3DRS_CULLMODE, _d3dRenderState.CullMode);
	dev->SetSamplerState(0, D3DSAMP_ADDRESSU, _d3dRenderState.SamplerState0U);
	dev->SetSamplerState(0, D3DSAMP_ADDRESSV, _d3dRenderState.SamplerState0V);
	dev->SetTransform(D3DTS_TEXTURE0, &_d3dRenderState.TexTransform0);
	dev->SetTransform(D3DTS_WORLD, &_d3dRenderState.World0);
	dev->SetRenderState(D3DRS_LIGHTING, _d3dRenderState.LightingEnabled);
	dev->SetRenderState(D3DRS_ALPHABLENDENABLE, _d3dRenderState.AlphaBlendEnabled);
	dev->SetTexture(0, _d3dRenderState.texture0);
	dev->SetTexture(1, _d3dRenderState.texture1);
	dev->SetTextureStageState(1, D3DTSS_COLOROP, _d3dRenderState.texture1ColoropState);
	dev->SetVertexShader(_d3dRenderState.vertexShader);
	dev->SetPixelShader(_d3dRenderState.pixelShader);
	_d3dRenderState.reset();
}

// ---------------------------------------
void RenderState::textureCreated(IDirect3DTexture9* tex) {
	unsigned int i;
	for (i = 0; i < _textureHandles.size(); ++i) {
		if (_textureHandles[i] == tex) {
			MM_LOG_INFO("Texture already found");
			break;
		}
	}
	if (i >= _textureHandles.size())
		_textureHandles.push_back(tex);
}

void RenderState::textureDeleted() {
	// TODO (hook) delete texture
}

void RenderState::setTextureStageState(DWORD Stage,D3DTEXTURESTAGESTATETYPE Type, DWORD Value) {
	if (Stage >= MM_MAX_STAGE) {
		MM_LOG_INFO(format("Warning: big stage: {}, state set to some value: {}", Stage, Value));
		return;
	}

	stageMap[Stage][Type] = Value;
}

void RenderState::setTexture(DWORD Stage,IDirect3DBaseTexture9* pTexture) {
	if (Stage >= MM_MAX_STAGE) {
		if (pTexture) {
			MM_LOG_INFO(format("Warning: big stage: {}, texture set to non null: {:x}", Stage, (int)pTexture));
		}
		return;
	}

	_stageEnabled[Stage] = pTexture != NULL;
	if (pTexture) {
		if (!_activeTextureLookup[pTexture]) {
			_activeTextureList.push_back(pTexture);
		}
		_activeTextureLookup[pTexture] = true;
	}

	bool isSelected = _currentTextureIdx != -1 && pTexture == _activeTextureList[_currentTextureIdx];
	_selectedOnStage[Stage] = isSelected;
}

void RenderState::saveTexture(int i, WCHAR* path) {
	LPDIRECT3DBASETEXTURE9 texture = NULL;
	if (FAILED(getDevice()->GetTexture(i, &texture))) {
		MM_LOG_INFO(format("Failed to query texture for stage: {}", i));
	} else if (!texture) {
		MM_LOG_INFO(format("Failed to obtain texture for stage {}; texture is null", i));
	} else {
		LPDIRECT3DBASETEXTURE9 snaptex = texture;
		if (texture == getSelectionTexture()) {
			// no point in snapping this
			snaptex = currentTexture();
		}

		// TODO: this will fail for textures in the default pool that are dynamic.  will probably need to force managed
		// pool when in snapshot mode for games that are affected by this.
		if (FAILED(D3DXSaveTextureToFileW(
			path,
			D3DXIFF_DDS,
			snaptex,
			NULL))) {
				MM_LOG_INFO(format("Failed to save texture {}", i));
		}
		texture->Release();
	}
}

void RenderState::setLight(DWORD Index,CONST D3DLIGHT9* light) {
	if (Index == 0) {
		if (light) {
			_lightZero = *light;
			_hasLightZero = true;
		} else
			_hasLightZero = false;
	}
}
void RenderState::getLight(DWORD Index,D3DLIGHT9** light) {
	Index;

	if (_hasLightZero)
		*light = &_lightZero;
	else
		*light = NULL;
}

};