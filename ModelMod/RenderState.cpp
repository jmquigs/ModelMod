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

#include "RenderState.h"
using namespace ModelMod;

#include <d3dx9tex.h>
#include "Util.h"

#include "Interop.h"

RenderState* RenderState::_sCurrentRenderState = NULL;
const string RenderState::LogCategory = "RenderState";

namespace ModelMod {

RenderState::RenderState(void) :
		_currentTextureIdx(-1),
		_currentTexturePtr(NULL),
		_snapRequested(false),
		_doingSnap(false),
		_snapStart(0),
		_initted(false),
		_showModMesh(false),
		_dipActive(false),
		_loadInProgress(false),
		_dev(NULL),
		_focusWindow(NULL),
		_selectionTexture(NULL),
		_currHookVB0(NULL),
		_pCurrentKeyMap(NULL)

	{
		_sCurrentRenderState = this;

		memset(_selectedOnStage, 0, sizeof(bool) * MM_MAX_STAGE);
		memset(_stageEnabled, 0, sizeof(bool) * MM_MAX_STAGE);

		InitNMB(_lastPixelShader);
	}

RenderState::~RenderState(void) {
		shutdown();
	}

void RenderState::shutdown() {
	MM_LOG_INFO("Releasing d3d resources");
	while (_d3dResources.size() > 0) {
		release(_d3dResources.begin()->first);
	}

	ReleaseNMB(_lastPixelShader);
}

void RenderState::clearLoadedMods() {
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
		if (mod.pixelShader) {
			release(mod.pixelShader);
		}
		for (Uint32 i = 0; i < MaxModTextures; ++i) {
			if (mod.texture[i]) {
				release(mod.texture[i]);
			}
		}
	}
	_managedMods.clear();

}
void RenderState::loadManagedAssembly() {
	clearLoadedMods();

	Interop::ReloadAssembly();
	if (Interop::OK()) {
		setKeyMap();
	}
}

void RenderState::loadMods() {
	if (!Interop::OK())
		return;

	if (_loadInProgress) {
		return;
	}

	int state = Interop::Callbacks().GetLoadingState();
	if (state == Code_AsyncLoadPending || state == Code_AsyncLoadInProgress) {
		return;
	}

	state = Interop::Callbacks().LoadModDB();
	if (state == Code_AsyncLoadPending || state == Code_AsyncLoadInProgress) {
		_loadInProgress = true;
		return;
	}
	else {
		setupModData();
	}
}

void RenderState::setupModData() {
	if (!Interop::OK()) {
		return;
	}

	int state = Interop::Callbacks().GetLoadingState();
	if (state != Code_AsyncLoadComplete) {
		MM_LOG_INFO(format("Error: setupModData called when loading state is not complete ({})", state));
		return;
	}

	_loadInProgress = false;

	clearLoadedMods();

	DWORD start, elapsed;
	start = GetTickCount();

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
			// fill was ok

			// create vertex declaration
			_dev->CreateVertexDeclaration((D3DVERTEXELEMENT9*)declData, &nModData.decl);
			if (nModData.decl) {
				this->add(nModData.decl);
			}

			int hashCode = NativeModData::hashCode(nModData.modData.refVertCount, nModData.modData.refPrimCount);

			// create textures 
			for (Uint32 i = 0; i < MaxModTextures; ++i) {
				if (wcslen(nModData.modData.texPath[i]) > 0) {
					IDirect3DTexture9 * tex = NULL;
					HRESULT hr = D3DXCreateTextureFromFileW(_dev, nModData.modData.texPath[i], &tex);
					if (FAILED(hr)) {
						MM_LOG_INFO(fmt::format("Error: failed to create mod texture for stage {}", i));
					}
					else {
						//char* mbName = Util::convertToMB(nModData.modData.texPath[i]);
						//MM_LOG_INFO(fmt::format("Created texture for stage {} from path {}", i, mbName));
						//delete[] mbName;
						nModData.texture[i] = tex;
						this->add(tex);
					}
				}
			}

			// create pixel shader if present
			if (wcslen(nModData.modData.pixelShaderPath) > 0) {

				Uint32 sizeBytes = 0;
				Uint8* psBytes = Util::slurpFile(nModData.modData.pixelShaderPath, sizeBytes);
				if (psBytes == NULL) {
					MM_LOG_INFO("Failed to read pixel shader file");
				}
				else {
					hr = _dev->CreatePixelShader((const DWORD *)psBytes, &nModData.pixelShader);
					if (FAILED(hr)) {
						MM_LOG_INFO("Failed to create pixel shader");
					}
					else {
						MM_LOG_INFO(format("Created pixel shader of size {}", sizeBytes));
						this->add(nModData.pixelShader);

						// disassemble and log for debugging
						//LPD3DXBUFFER buf;

						//if (FAILED(D3DXDisassembleShader((const DWORD *)psBytes, FALSE, NULL, &buf))) {
						//	MM_LOG_INFO("failed to disassemble loaded shader");
						//}
						//else {
						//	string s((const char*)buf->GetBufferPointer(), buf->GetBufferSize());
						//	MM_LOG_INFO("Loaded shader:");
						//	MM_LOG_INFO(format("{}", s));
						//}
					}

					// TODO: I _think_ that CreatePixelShader is copying the data, so this is safe.  wish the docs spelled it out.
					delete[] psBytes;
				}
			}

			// store in mod DB
			_managedMods[hashCode] = nModData; // structwise-copy is ok
		}

		delete [] declData; // TODO: another case where the docs don't specify if the declaration copies this.  I think it does.
	}


	elapsed = GetTickCount() - start;
	MM_LOG_INFO(format("Mod Data Setup time (Native+Managed): {}", elapsed));
}

void RenderState::loadEverything() {
	loadManagedAssembly();
	loadMods();
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

	// create "selected" texture
	WCHAR* dataPath = NULL;
	if (Interop::OK()) {
		IDirect3DTexture9 * tex;

		int width = 256;
		int height = 256;
		HRESULT hr = dev->CreateTexture(width, height, 1, 0,
			D3DFMT_A8R8G8B8, D3DPOOL_MANAGED, &tex, 0);
		if (FAILED(hr)) {
			MM_LOG_INFO("Failed to create 'selection' texture");
		} else {
			_selectionTexture = tex;

			D3DLOCKED_RECT rect;
			hr = tex->LockRect(0, &rect, 0, D3DLOCK_DISCARD);
			if (FAILED(hr)) {
				MM_LOG_INFO("Failed to lock 'selection' texture");
			}
			else {
				unsigned char* dest = static_cast<unsigned char*>(rect.pBits);

				// fill it with a lovely shade of green
				Uint32 numEls = width * height;
				for (Uint32 i = 0; i < numEls; ++i) {
					Uint32* d = (Uint32*)(dest + (i*sizeof(Uint32)));
					*d = 0xFF00FF00;
				}
				//MM_LOG_INFO("filled selection texture");
				tex->UnlockRect(0);
			}
		}
	}

	// Set key bindings.  Input also assumes that CONTROL modifier is required for these as well.
	// TODO: should push this out to conf file eventually so that they can be customized without rebuild
	// If you change these, be sure to change LocStrings/ProfileText in MMLaunch!
	_punctKeyMap[DIK_BACKSLASH] = [&]() { this->loadMods(); }; 
	_punctKeyMap[DIK_RBRACKET] = [&]() { this->toggleShowModMesh(); };
	_punctKeyMap[DIK_SEMICOLON] = [&]() { this->clearTextureLists(); };
	_punctKeyMap[DIK_COMMA] = [&]() { this->selectNextTexture(); };
	_punctKeyMap[DIK_PERIOD] = [&]() { this->selectPrevTexture(); };
	_punctKeyMap[DIK_SLASH] = [&]() { this->requestSnap(); };
	_punctKeyMap[DIK_MINUS] = [&]() { this->loadEverything(); }; 
	

	// If you change these, be sure to change LocStrings/ProfileText in MMLaunch!
	_fKeyMap[DIK_F1] = [&]() { this->loadMods(); };
	_fKeyMap[DIK_F2] = [&]() { this->toggleShowModMesh(); };
	_fKeyMap[DIK_F6] = [&]() { this->clearTextureLists(); };
	_fKeyMap[DIK_F3] = [&]() { this->selectNextTexture(); };
	_fKeyMap[DIK_F4] = [&]() { this->selectPrevTexture(); };
	_fKeyMap[DIK_F7] = [&]() { this->requestSnap(); };
	_fKeyMap[DIK_F10] = [&]() { this->loadEverything(); };

	_pCurrentKeyMap = &_fKeyMap;
	
	if (Interop::OK()) {
		if (Interop::Conf().LoadModsOnStart) {
			loadEverything();
			toggleShowModMesh();
		}
		else {
			loadManagedAssembly();
		}
	}

	_initted = true;
}

void RenderState::setKeyMap() {
	if (Interop::OK()) {
		string iprofile = Util::toLowerCase(Interop::Conf().InputProfile);
		if (Util::startsWith(iprofile, "punct")) {
			_pCurrentKeyMap = &_punctKeyMap;
		}
		else if (Util::startsWith(iprofile, "fk")) {
			_pCurrentKeyMap = &_fKeyMap;
		}
	}
}

void RenderState::addSceneNotify(ISceneNotify* notify) {
	if (!notify)
		return;
	_sceneNotify.push_back(notify);
}

static const bool SnapWholeScene = false;
// Snapshotting currently stops after a certain amount of real time has passed from the start of the snap, specified by this constant.
// One might expect that just snapping everything drawn within a single begin/end scene combo is sufficient, but this often misses data, 
// and sometimes fails to snapshot anything at all.  Perhaps the game is using multiple being/end combos, or drawing outside of begin/end 
// block.  Using a window makes it much more likely that something useful is captured, at the expense of some duplicates; even though
// some objects may still be missed.  Some investigation to make this more reliable would be useful.
static const ULONGLONG SnapMS = 250;

void RenderState::beginScene(IDirect3DDevice9* dev) {
	if (!_initted)
		init(dev);

	if (dev != _dev) {
		MM_LOG_INFO("Warning: device changed in beginScene; render state may not handle this case");
		// Never seen this happen, but if it does at some point, then we need to re-create resources and track the new device.
	}

	// process input only when the d3d window is in the foreground.  this style of processing creates issues for keyup processing, 
	// since we can lose events, but we don't do any of that currently.
	bool inputOk = false;
	if (_pCurrentKeyMap && _focusWindow != NULL) {
		HWND focused = GetForegroundWindow();
		inputOk = focused == _focusWindow;
		if (!inputOk) {
			// check parent
			HWND par = GetParent(_focusWindow);
			inputOk = par == focused;
		}
		if (!inputOk) {
			// check root owner
			HWND own = GetAncestor(_focusWindow, GA_ROOTOWNER);
			inputOk = own == focused;
		}
	}

	if (inputOk) {
		vector<Input::KeyEvent> events = _input.update();

		for (Uint32 i = 0; i < events.size(); ++i) {
			Input::KeyEvent& evt = events[i];

			//MM_LOG_INFO(format("event: key: {}; pressed: {}; mapped: {}; modctrl: {}", 
			//	evt.key, 
			//	evt.pressed, 
			//	(_pCurrentKeyMap->count(evt.key) > 0),
			//	_input.isCtrlPressed()));

			if (evt.pressed) {
				if (_input.isCtrlPressed()) {
					if (_pCurrentKeyMap->count(evt.key) > 0) {
						(*_pCurrentKeyMap)[evt.key]();
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

	if (Interop::OK()) {
		if (_loadInProgress && Interop::Callbacks().GetLoadingState() == Code_AsyncLoadComplete) {
			setupModData();
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

	//MM_LOG_INFO(format("Current texture set to: {:x}", (int)_currentTexturePtr));
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

	//MM_LOG_INFO(format("Current texture set to: {:x}", (int)_currentTexturePtr));
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
	for (Uint32 i = 0; i < MaxModTextures; ++i) {
		dev->GetTexture(i, &_d3dRenderState.texture[i]); // definitely release
	}
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
	for (Uint32 i = 0; i < MaxModTextures; ++i) {
		dev->SetTexture(i, _d3dRenderState.texture[i]); 
	}
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
	// Not required right now since we just let our texture handles go stale, and the active list can be cleared easily.
	// Anyway we don't even have a hook object for textures, and we can't hook the deletion without that.
}

void RenderState::setTextureStageState(DWORD Stage,D3DTEXTURESTAGESTATETYPE Type, DWORD Value) {
	if (Stage >= MM_MAX_STAGE) {
		MM_LOG_INFO_LIMIT(format("Warning: big stage: {}, state set to some value: {}", Stage, Value), 10);
		return;
	}

	stageMap[Stage][Type] = Value;
}

void RenderState::setTexture(DWORD Stage,IDirect3DBaseTexture9* pTexture) {
	if (Stage >= MM_MAX_STAGE) {
		if (pTexture) {
			MM_LOG_INFO_LIMIT(format("Warning: big stage: {}, texture set to non null: {:x}", Stage, (int)pTexture), 10);
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

bool RenderState::saveTexture(int i, WCHAR* path) {
	LPDIRECT3DBASETEXTURE9 texture = NULL;
	if (FAILED(getDevice()->GetTexture(i, &texture))) {
		MM_LOG_INFO(format("Failed to query texture for stage: {}", i));
		return false;
	} else if (!texture) {
		MM_LOG_INFO(format("Failed to obtain texture for stage {}; texture is null", i));
		return false;
	} else {
		LPDIRECT3DBASETEXTURE9 snaptex = texture;
		if (texture == getSelectionTexture()) {
			// no point in snapping this
			snaptex = currentTexture();
		}

		// TODO: this will fail for textures in the default pool that are dynamic.  will probably need to force managed
		// pool when in snapshot mode for games that are affected by this.
		bool ok = SUCCEEDED(D3DXSaveTextureToFileW(
			path,
			D3DXIFF_DDS,
			snaptex,
			NULL));

		if (!ok) {
			MM_LOG_INFO(format("Failed to save texture {}", i));
		}
		texture->Release();

		return ok;		
	}
}

NativeMemoryBuffer RenderState::getPixelShader() {
	LPDIRECT3DPIXELSHADER9 shader = NULL;
	UINT size = 0;

	ReleaseNMB(_lastPixelShader);

	InvokeOnDrop drop([&]() {
		if (shader) {
			MM_LOG_INFO("disposing shader");
		}
		SAFE_RELEASE(shader);
	});

	if (FAILED(getDevice()->GetPixelShader(&shader))) {
		MM_LOG_INFO(format("Failed to save pixel shader"));
		return _lastPixelShader;
	}

	if (shader == NULL) {
		MM_LOG_INFO(format("No pixel shader present"));
		return _lastPixelShader;
	}

	if (FAILED(shader->GetFunction(NULL, &size))) {
		MM_LOG_INFO(format("Failed to get pixel shader size"));
		return _lastPixelShader;
	}

	// Just to prevent overflow, but of course, its not reasonable to allocate a 2GB array for a pixel shader
	if (size > INT_MAX) { 
		MM_LOG_INFO(format("Failed to get pixel shader data; size is too large: {}", size));
		return _lastPixelShader;
	}

	AllocNMB(_lastPixelShader, size);

	if (FAILED(shader->GetFunction(_lastPixelShader.data, &size))) {
		MM_LOG_INFO(format("Failed to get pixel shader data"));
		ReleaseNMB(_lastPixelShader);
		return _lastPixelShader;
	}

	// Disassemble and log for debugging		
	//LPD3DXBUFFER buf;

	//if (FAILED(D3DXDisassembleShader((const DWORD *)data, FALSE, NULL, &buf))) {
	//	MM_LOG_INFO("failed to disassemble snapshot shader");
	//}
	//else {
	//	string s((const char*)buf->GetBufferPointer(), buf->GetBufferSize());
	//	MM_LOG_INFO("Snapshot shader:");
	//	MM_LOG_INFO(format("{}", s));
	//}

	return _lastPixelShader;
}

};