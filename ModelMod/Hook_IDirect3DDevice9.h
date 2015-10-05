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

#include <d3d9.h>
#include <d3dx9math.h>

#include "Hook_IDirect3DVertexBuffer9.h"

#include "RenderState.h"
#include "Log.h"
using namespace ModelMod;

//#define D3D_CALL_LOG(s) MM_LOG_INFO(s)
#define D3D_CALL_LOG(s)

// Its a significant CPU hit to track shader constants, and snapshot doesn't even 
// write them out at the moment, so its wasted effort.  
// Can revisit this if and when I decide to implement shader modding.
#define TRACK_SHADER_CONSTANTS 0

#define MaxActiveStreams 32
class Hook_IDirect3DDevice9 : public IDirect3DDevice9 {
	static const string LogCategory;

	IDirect3DDevice9* _dev;
	IDirect3D9* _d3d9;
	ModelMod::RenderState _hookRenderState;
	int _refCount;

	IDirect3DVertexBuffer9* _currStream0VB;
	IDirect3DIndexBuffer9* _currIndices;
	
	bool activeStreams[MaxActiveStreams];

public:
	Hook_IDirect3DDevice9(IDirect3D9* d3d9,IDirect3DDevice9* dev) :
		_dev(dev),
		_d3d9(d3d9),
		_refCount(0),
		_currStream0VB(NULL),
		_currIndices(NULL) {
			for (int i = 0; i < MaxActiveStreams; ++i) {
				activeStreams[i] = false;
			}
	}

	virtual ~Hook_IDirect3DDevice9(void) {
	}

	/*** IUnknown methods ***/
	STDMETHOD(QueryInterface)(THIS_ REFIID riid, void** ppvObj) {
		D3D_CALL_LOG("d3d QueryInterface");
		return _dev->QueryInterface(riid,ppvObj);
	}

	STDMETHOD_(ULONG,AddRef)(THIS) {
		D3D_CALL_LOG("d3d AddRef");
		_refCount++;
		DWORD devRefCount = _dev->AddRef();
		MM_LOG_INFO(fmt::format("AddRef: hook count: {0}, dev count: {1}", _refCount, devRefCount));
		return devRefCount;
	}
	STDMETHOD_(ULONG,Release)(THIS) {
		D3D_CALL_LOG("d3d Release");
		if (_refCount <= 3) {
			MM_LOG_INFO(fmt::format("hook device prerelease: {}", _refCount));
		}
		if (_refCount <= 0)
			_hookRenderState.shutdown();

		_refCount--;
		if (_refCount < 0) {
			_refCount = 0;
		}

		DWORD devRefCount = _dev->Release();

		MM_LOG_INFO(fmt::format("Release: hook count: {}, dev count: {}", _refCount, devRefCount));

		return devRefCount;
	}

	/*** IDirect3DDevice9 methods ***/
	STDMETHOD(TestCooperativeLevel)(THIS) {
		D3D_CALL_LOG("d3d TestCooperativeLevel");
		return _dev->TestCooperativeLevel();
	}
	STDMETHOD_(UINT, GetAvailableTextureMem)(THIS) {
		D3D_CALL_LOG("d3d GetAvailableTextureMem");
		return _dev->GetAvailableTextureMem();
	}
	STDMETHOD(EvictManagedResources)(THIS) {
		D3D_CALL_LOG("d3d EvictManagedResources");
		return _dev->EvictManagedResources();
	}
	STDMETHOD(GetDirect3D)(THIS_ IDirect3D9** ppD3D9) {
		D3D_CALL_LOG("d3d GetDirect3D");
		// TODO (hook): should be returning hook d3d9 here, but doing so crashes DXUT.  Why?
		//*ppD3D9 = _d3d9;
		//return D3D_OK;
		return _dev->GetDirect3D(ppD3D9);
	}
	STDMETHOD(GetDeviceCaps)(THIS_ D3DCAPS9* pCaps) {
		D3D_CALL_LOG("d3d GetDeviceCaps");
		return _dev->GetDeviceCaps(pCaps);
	}
	STDMETHOD(GetDisplayMode)(THIS_ UINT iSwapChain,D3DDISPLAYMODE* pMode) {
		D3D_CALL_LOG("d3d GetDisplayMode");
		return _dev->GetDisplayMode(iSwapChain,pMode);
	}
	STDMETHOD(GetCreationParameters)(THIS_ D3DDEVICE_CREATION_PARAMETERS *pParameters) {
		D3D_CALL_LOG("d3d GetCreationParameters");
		return _dev->GetCreationParameters(pParameters);
	}
	STDMETHOD(SetCursorProperties)(THIS_ UINT XHotSpot,UINT YHotSpot,IDirect3DSurface9* pCursorBitmap) {
		D3D_CALL_LOG("d3d SetCursorProperties");
		return _dev->SetCursorProperties(XHotSpot,YHotSpot,pCursorBitmap);
	}
	STDMETHOD_(void, SetCursorPosition)(THIS_ int X,int Y,DWORD Flags) {
		D3D_CALL_LOG("d3d SetCursorPosition");
		return _dev->SetCursorPosition(X,Y,Flags);
	}
	STDMETHOD_(BOOL, ShowCursor)(THIS_ BOOL bShow) {
		D3D_CALL_LOG("d3d ShowCursor");
		return _dev->ShowCursor(bShow);
	}
	STDMETHOD(CreateAdditionalSwapChain)(THIS_ D3DPRESENT_PARAMETERS* pPresentationParameters,IDirect3DSwapChain9** pSwapChain) {
		D3D_CALL_LOG("d3d CreateAdditionalSwapChain");
		return _dev->CreateAdditionalSwapChain(pPresentationParameters,pSwapChain);
	}
	STDMETHOD(GetSwapChain)(THIS_ UINT iSwapChain,IDirect3DSwapChain9** pSwapChain) {
		D3D_CALL_LOG("d3d GetSwapChain");
		return _dev->GetSwapChain(iSwapChain,pSwapChain);
	}
	STDMETHOD_(UINT, GetNumberOfSwapChains)(THIS) {
		D3D_CALL_LOG("d3d GetNumberOfSwapChains");
		return _dev->GetNumberOfSwapChains();
	}
	STDMETHOD(Reset)(THIS_ D3DPRESENT_PARAMETERS* pPresentationParameters) {
		D3D_CALL_LOG("d3d Reset");
		_hookRenderState.vsFloatConstants.clear();
		_hookRenderState.psFloatConstants.clear();

		// TODO (device reset): other cleanup here?

		return _dev->Reset(pPresentationParameters);
	}
	STDMETHOD(Present)(THIS_ CONST RECT* pSourceRect,CONST RECT* pDestRect,HWND hDestWindowOverride,CONST RGNDATA* pDirtyRegion) {
		D3D_CALL_LOG("d3d Present");
		return _dev->Present(pSourceRect,pDestRect,hDestWindowOverride,pDirtyRegion);
	}
	STDMETHOD(GetBackBuffer)(THIS_ UINT iSwapChain,UINT iBackBuffer,D3DBACKBUFFER_TYPE Type,IDirect3DSurface9** ppBackBuffer) {
		D3D_CALL_LOG("d3d GetBackBuffer");
		return _dev->GetBackBuffer(iSwapChain,iBackBuffer,Type,ppBackBuffer);
	}
	STDMETHOD(GetRasterStatus)(THIS_ UINT iSwapChain,D3DRASTER_STATUS* pRasterStatus) {
		D3D_CALL_LOG("d3d GetRasterStatus");
		return _dev->GetRasterStatus(iSwapChain, pRasterStatus);
	}
	STDMETHOD(SetDialogBoxMode)(THIS_ BOOL bEnableDialogs) {
		D3D_CALL_LOG("d3d SetDialogBoxMode");
		return _dev->SetDialogBoxMode(bEnableDialogs);
	}
	STDMETHOD_(void, SetGammaRamp)(THIS_ UINT iSwapChain,DWORD Flags,CONST D3DGAMMARAMP* pRamp) {
		D3D_CALL_LOG("d3d SetGammaRamp");
		return _dev->SetGammaRamp(iSwapChain,Flags,pRamp);
	}
	STDMETHOD_(void, GetGammaRamp)(THIS_ UINT iSwapChain,D3DGAMMARAMP* pRamp) {
		D3D_CALL_LOG("d3d GetGammaRamp");
		return _dev->GetGammaRamp(iSwapChain,pRamp);
	}
	STDMETHOD(CreateTexture)(THIS_ UINT Width,UINT Height,UINT Levels,DWORD Usage,D3DFORMAT Format,D3DPOOL Pool,IDirect3DTexture9** ppTexture,HANDLE* pSharedHandle) {
		D3D_CALL_LOG("d3d CreateTexture");
		HRESULT hr = _dev->CreateTexture(Width,Height,Levels,Usage,Format,Pool,ppTexture,pSharedHandle);

		// technically, to track these properly I should hook IDirect3DTexture9, 
		// otherwise we won't be able to tell when the texture is deleted.  However, if I'm just using 
		// them for pointer comparisons, maybe its ok to let them go stale; depends on the usage.
		// Anyway, I don't use this right now and its spammy in the logs, so I'm disabling it.
		//if (SUCCEEDED(hr) && Levels >= 7) {
		//	 
		//	string texInfoStr = fmt::format("Texture {:x} for tex {}x{}, {} levels, {} format", (int)*ppTexture, Width, Height, Levels, Format);
		//	
		//	MM_LOG_INFO("Creating: " + texInfoStr);
		//	_hookRenderState.textureCreated(*ppTexture);
		//	_hookRenderState.texInfo[(DWORD)*ppTexture] = texInfoStr;
		//}

		return hr;
	}
	STDMETHOD(CreateVolumeTexture)(THIS_ UINT Width,UINT Height,UINT Depth,UINT Levels,DWORD Usage,D3DFORMAT Format,D3DPOOL Pool,IDirect3DVolumeTexture9** ppVolumeTexture,HANDLE* pSharedHandle) {
		D3D_CALL_LOG("d3d CreateVolumeTexture");
		return _dev->CreateVolumeTexture(Width,Height,Depth,Levels,Usage,Format,Pool,ppVolumeTexture,pSharedHandle);
	}
	STDMETHOD(CreateCubeTexture)(THIS_ UINT EdgeLength,UINT Levels,DWORD Usage,D3DFORMAT Format,D3DPOOL Pool,IDirect3DCubeTexture9** ppCubeTexture,HANDLE* pSharedHandle) {
		D3D_CALL_LOG("d3d CreateCubeTexture");
		return _dev->CreateCubeTexture(EdgeLength,Levels,Usage,Format,Pool,ppCubeTexture,pSharedHandle);
	}
	STDMETHOD(CreateVertexBuffer)(THIS_ UINT Length,DWORD Usage,DWORD FVF,D3DPOOL Pool,IDirect3DVertexBuffer9** ppVertexBuffer,HANDLE* pSharedHandle) {
		D3D_CALL_LOG("d3d CreateVertexBuffer");
		//Usage = Usage & (~D3DUSAGE_WRITEONLY);
		//Usage = Usage & (~D3DUSAGE_DYNAMIC);
		HRESULT hr = _dev->CreateVertexBuffer(Length,Usage,FVF,Pool,ppVertexBuffer,pSharedHandle);
		if (SUCCEEDED(hr) && MM_HOOK_VERTEX_BUFFERS) {
			Hook_IDirect3DVertexBuffer9* hvb = new Hook_IDirect3DVertexBuffer9(*ppVertexBuffer,Length);
			*ppVertexBuffer = hvb;
			//MM_LOG_INFO(format("Create VB: %08X, len %d usage %d fvf %d pool %d") % (DWORD)*ppVertexBuffer % Length % Usage % FVF % Pool);
		}
		return hr;
	}
	STDMETHOD(CreateIndexBuffer)(THIS_ UINT Length,DWORD Usage,D3DFORMAT Format,D3DPOOL Pool,IDirect3DIndexBuffer9** ppIndexBuffer,HANDLE* pSharedHandle) {
		D3D_CALL_LOG("d3d CreateIndexBuffer");
		Usage = Usage & (~D3DUSAGE_WRITEONLY);
		//Usage = Usage & (~D3DUSAGE_DYNAMIC);
		HRESULT hr = _dev->CreateIndexBuffer(Length,Usage,Format,Pool,ppIndexBuffer,pSharedHandle);
		if (SUCCEEDED(hr)) {
			//MM_LOG_INFO(format("Create IB: %08X, len %d usage %d pool %d") % (DWORD)*ppIndexBuffer % Length % Usage % Pool);
		}
		return hr;
	}
	STDMETHOD(CreateRenderTarget)(THIS_ UINT Width,UINT Height,D3DFORMAT Format,D3DMULTISAMPLE_TYPE MultiSample,DWORD MultisampleQuality,BOOL Lockable,IDirect3DSurface9** ppSurface,HANDLE* pSharedHandle) {
		D3D_CALL_LOG("d3d CreateRenderTarget");
		return _dev->CreateRenderTarget(Width,Height,Format,MultiSample,MultisampleQuality,Lockable,ppSurface,pSharedHandle);
	}
	STDMETHOD(CreateDepthStencilSurface)(THIS_ UINT Width,UINT Height,D3DFORMAT Format,D3DMULTISAMPLE_TYPE MultiSample,DWORD MultisampleQuality,BOOL Discard,IDirect3DSurface9** ppSurface,HANDLE* pSharedHandle) {
		D3D_CALL_LOG("d3d CreateDepthStencilSurface");
		return _dev->CreateDepthStencilSurface(Width,Height,Format,MultiSample,MultisampleQuality,Discard,ppSurface,pSharedHandle);
	}
	STDMETHOD(UpdateSurface)(THIS_ IDirect3DSurface9* pSourceSurface,CONST RECT* pSourceRect,IDirect3DSurface9* pDestinationSurface,CONST POINT* pDestPoint) {
		D3D_CALL_LOG("d3d UpdateSurface");
		return _dev->UpdateSurface(pSourceSurface,pSourceRect,pDestinationSurface,pDestPoint);
	}
	STDMETHOD(UpdateTexture)(THIS_ IDirect3DBaseTexture9* pSourceTexture,IDirect3DBaseTexture9* pDestinationTexture) {
		D3D_CALL_LOG("d3d UpdateTexture");
		return _dev->UpdateTexture(pSourceTexture,pDestinationTexture);
	}
	STDMETHOD(GetRenderTargetData)(THIS_ IDirect3DSurface9* pRenderTarget,IDirect3DSurface9* pDestSurface) {
		D3D_CALL_LOG("d3d GetRenderTargetData");
		return _dev->GetRenderTargetData(pRenderTarget,pDestSurface);
	}
	STDMETHOD(GetFrontBufferData)(THIS_ UINT iSwapChain,IDirect3DSurface9* pDestSurface) {
		D3D_CALL_LOG("d3d GetFrontBufferData");
		return _dev->GetFrontBufferData(iSwapChain,pDestSurface);
	}
	STDMETHOD(StretchRect)(THIS_ IDirect3DSurface9* pSourceSurface,CONST RECT* pSourceRect,IDirect3DSurface9* pDestSurface,CONST RECT* pDestRect,D3DTEXTUREFILTERTYPE Filter) {
		D3D_CALL_LOG("d3d StretchRect");
		return _dev->StretchRect(pSourceSurface,pSourceRect,pDestSurface,pDestRect,Filter);
	}
	STDMETHOD(ColorFill)(THIS_ IDirect3DSurface9* pSurface,CONST RECT* pRect,D3DCOLOR color) {
		D3D_CALL_LOG("d3d ColorFill");
		return _dev->ColorFill(pSurface,pRect,color);
	}
	STDMETHOD(CreateOffscreenPlainSurface)(THIS_ UINT Width,UINT Height,D3DFORMAT Format,D3DPOOL Pool,IDirect3DSurface9** ppSurface,HANDLE* pSharedHandle) {
		D3D_CALL_LOG("d3d CreateOffscreenPlainSurface");
		return _dev->CreateOffscreenPlainSurface(Width,Height,Format,Pool,ppSurface,pSharedHandle);
	}
	STDMETHOD(SetRenderTarget)(THIS_ DWORD RenderTargetIndex,IDirect3DSurface9* pRenderTarget) {
		D3D_CALL_LOG("d3d SetRenderTarget");
		return _dev->SetRenderTarget(RenderTargetIndex,pRenderTarget);
	}
	STDMETHOD(GetRenderTarget)(THIS_ DWORD RenderTargetIndex,IDirect3DSurface9** ppRenderTarget) {
		D3D_CALL_LOG("d3d GetRenderTarget");
		return _dev->GetRenderTarget(RenderTargetIndex,ppRenderTarget);
	}
	STDMETHOD(SetDepthStencilSurface)(THIS_ IDirect3DSurface9* pNewZStencil) {
		D3D_CALL_LOG("d3d SetDepthStencilSurface");
		return _dev->SetDepthStencilSurface(pNewZStencil);
	}
	STDMETHOD(GetDepthStencilSurface)(THIS_ IDirect3DSurface9** ppZStencilSurface) {
		D3D_CALL_LOG("d3d GetDepthStencilSurface");
		return _dev->GetDepthStencilSurface(ppZStencilSurface);
	}

	STDMETHOD(BeginScene)(THIS) {
		D3D_CALL_LOG("d3d BeginScene");
		_hookRenderState.beginScene(this);

		return _dev->BeginScene();
	}
	STDMETHOD(EndScene)(THIS) {
		D3D_CALL_LOG("d3d EndScene");

		_hookRenderState.endScene(this);

		return _dev->EndScene();
	}
	STDMETHOD(Clear)(THIS_ DWORD Count,CONST D3DRECT* pRects,DWORD Flags,D3DCOLOR Color,float Z,DWORD Stencil) {
		D3D_CALL_LOG("d3d Clear");
		return _dev->Clear(Count,pRects,Flags,Color,Z,Stencil);
	}
	STDMETHOD(SetTransform)(THIS_ D3DTRANSFORMSTATETYPE State,CONST D3DMATRIX* pMatrix) {
		D3D_CALL_LOG("d3d SetTransform");
		return _dev->SetTransform(State,pMatrix);
	}
	STDMETHOD(GetTransform)(THIS_ D3DTRANSFORMSTATETYPE State,D3DMATRIX* pMatrix) {
		D3D_CALL_LOG("d3d GetTransform");
		return _dev->GetTransform(State,pMatrix);
	}
	STDMETHOD(MultiplyTransform)(THIS_ D3DTRANSFORMSTATETYPE State,CONST D3DMATRIX* matrix) {
		D3D_CALL_LOG("d3d MultiplyTransform");
		return _dev->MultiplyTransform(State, matrix);
	}
	STDMETHOD(SetViewport)(THIS_ CONST D3DVIEWPORT9* pViewport) {
		D3D_CALL_LOG("d3d SetViewport");
		return _dev->SetViewport(pViewport);
	}
	STDMETHOD(GetViewport)(THIS_ D3DVIEWPORT9* pViewport) {
		D3D_CALL_LOG("d3d GetViewport");
		return _dev->GetViewport(pViewport);
	}
	STDMETHOD(SetMaterial)(THIS_ CONST D3DMATERIAL9* pMaterial) {
		D3D_CALL_LOG("d3d SetMaterial");
		return _dev->SetMaterial(pMaterial);
	}
	STDMETHOD(GetMaterial)(THIS_ D3DMATERIAL9* pMaterial) {
		D3D_CALL_LOG("d3d GetMaterial");
		return _dev->GetMaterial(pMaterial);
	}
	STDMETHOD(SetLight)(THIS_ DWORD Index,CONST D3DLIGHT9* light) {
		D3D_CALL_LOG("d3d SetLight");
		HRESULT hr = _dev->SetLight(Index, light);
		if (SUCCEEDED(hr)) {
			_hookRenderState.setLight(Index,light);
		}
		return hr;
	}
	STDMETHOD(GetLight)(THIS_ DWORD Index,D3DLIGHT9* light) {
		D3D_CALL_LOG("d3d GetLight");
		return _dev->GetLight(Index,light);
	}
	STDMETHOD(LightEnable)(THIS_ DWORD Index,BOOL Enable) {
		D3D_CALL_LOG("d3d LightEnable");
		return _dev->LightEnable(Index,Enable);
	}
	STDMETHOD(GetLightEnable)(THIS_ DWORD Index,BOOL* pEnable) {
		D3D_CALL_LOG("d3d GetLightEnable");
		return _dev->GetLightEnable(Index,pEnable);
	}
	STDMETHOD(SetClipPlane)(THIS_ DWORD Index,CONST float* pPlane) {
		D3D_CALL_LOG("d3d SetClipPlane");
		return _dev->SetClipPlane(Index,pPlane);
	}
	STDMETHOD(GetClipPlane)(THIS_ DWORD Index,float* pPlane) {
		D3D_CALL_LOG("d3d GetClipPlane");
		return _dev->GetClipPlane(Index,pPlane);
	}
	STDMETHOD(SetRenderState)(THIS_ D3DRENDERSTATETYPE State,DWORD Value) {
		D3D_CALL_LOG("d3d SetRenderState");
		return _dev->SetRenderState(State,Value);
	}
	STDMETHOD(GetRenderState)(THIS_ D3DRENDERSTATETYPE State,DWORD* pValue) {
		D3D_CALL_LOG("d3d GetRenderState");
		return _dev->GetRenderState(State,pValue);
	}
	STDMETHOD(CreateStateBlock)(THIS_ D3DSTATEBLOCKTYPE Type,IDirect3DStateBlock9** ppSB) {
		D3D_CALL_LOG("d3d CreateStateBlock");
		return _dev->CreateStateBlock(Type,ppSB);
	}
	STDMETHOD(BeginStateBlock)(THIS) {
		D3D_CALL_LOG("d3d BeginStateBlock");
		return _dev->BeginStateBlock();
	}
	STDMETHOD(EndStateBlock)(THIS_ IDirect3DStateBlock9** ppSB) {
		D3D_CALL_LOG("d3d EndStateBlock");
		return _dev->EndStateBlock(ppSB);
	}
	STDMETHOD(SetClipStatus)(THIS_ CONST D3DCLIPSTATUS9* pClipStatus) {
		D3D_CALL_LOG("d3d SetClipStatus");
		return _dev->SetClipStatus(pClipStatus);
	}
	STDMETHOD(GetClipStatus)(THIS_ D3DCLIPSTATUS9* pClipStatus) {
		D3D_CALL_LOG("d3d GetClipStatus");
		return _dev->GetClipStatus(pClipStatus);
	}
	STDMETHOD(GetTexture)(THIS_ DWORD Stage,IDirect3DBaseTexture9** ppTexture) {
		D3D_CALL_LOG("d3d GetTexture");
		return _dev->GetTexture(Stage,ppTexture);
	}
	STDMETHOD(SetTexture)(THIS_ DWORD Stage,IDirect3DBaseTexture9* pTexture) {
		D3D_CALL_LOG("d3d SetTexture");
		HRESULT hr = _dev->SetTexture(Stage,pTexture);
		if (SUCCEEDED(hr))
			_hookRenderState.setTexture(Stage,pTexture);
		return hr;
	}
	STDMETHOD(GetTextureStageState)(THIS_ DWORD Stage,D3DTEXTURESTAGESTATETYPE Type,DWORD* pValue) {
		D3D_CALL_LOG("d3d GetTextureStageState");
		return _dev->GetTextureStageState(Stage,Type,pValue);
	}
	STDMETHOD(SetTextureStageState)(THIS_ DWORD Stage,D3DTEXTURESTAGESTATETYPE Type,DWORD Value) {
		D3D_CALL_LOG("d3d SetTextureStageState");
		return _dev->SetTextureStageState(Stage,Type,Value);
	}
	STDMETHOD(GetSamplerState)(THIS_ DWORD Sampler,D3DSAMPLERSTATETYPE Type,DWORD* pValue) {
		D3D_CALL_LOG("d3d GetSamplerState");
		return _dev->GetSamplerState(Sampler,Type,pValue);
	}
	STDMETHOD(SetSamplerState)(THIS_ DWORD Sampler,D3DSAMPLERSTATETYPE Type,DWORD Value) {
		D3D_CALL_LOG("d3d SetSamplerState");
		return _dev->SetSamplerState(Sampler,Type,Value);
	}
	STDMETHOD(ValidateDevice)(THIS_ DWORD* pNumPasses) {
		D3D_CALL_LOG("d3d ValidateDevice");
		return _dev->ValidateDevice(pNumPasses);
	}
	STDMETHOD(SetPaletteEntries)(THIS_ UINT PaletteNumber,CONST PALETTEENTRY* pEntries) {
		D3D_CALL_LOG("d3d SetPaletteEntries");
		return _dev->SetPaletteEntries(PaletteNumber,pEntries);
	}
	STDMETHOD(GetPaletteEntries)(THIS_ UINT PaletteNumber,PALETTEENTRY* pEntries) {
		D3D_CALL_LOG("d3d GetPaletteEntries");
		return _dev->GetPaletteEntries(PaletteNumber,pEntries);
	}
	STDMETHOD(SetCurrentTexturePalette)(THIS_ UINT PaletteNumber) {
		D3D_CALL_LOG("d3d SetCurrentTexturePalette");
		return _dev->SetCurrentTexturePalette(PaletteNumber);
	}
	STDMETHOD(GetCurrentTexturePalette)(THIS_ UINT *PaletteNumber) {
		D3D_CALL_LOG("d3d GetCurrentTexturePalette");
		return _dev->GetCurrentTexturePalette(PaletteNumber);
	}
	STDMETHOD(SetScissorRect)(THIS_ CONST RECT* pRect) {
		D3D_CALL_LOG("d3d SetScissorRect");
		return _dev->SetScissorRect(pRect);
	}
	STDMETHOD(GetScissorRect)(THIS_ RECT* pRect) {
		D3D_CALL_LOG("d3d GetScissorRect");
		return _dev->GetScissorRect(pRect);
	}
	STDMETHOD(SetSoftwareVertexProcessing)(THIS_ BOOL bSoftware) {
		D3D_CALL_LOG("d3d SetSoftwareVertexProcessing");
		return _dev->SetSoftwareVertexProcessing(bSoftware);
	}
	STDMETHOD_(BOOL, GetSoftwareVertexProcessing)(THIS) {
		D3D_CALL_LOG("d3d GetSoftwareVertexProcessing");
		return _dev->GetSoftwareVertexProcessing();
	}
	STDMETHOD(SetNPatchMode)(THIS_ float nSegments) {
		D3D_CALL_LOG("d3d SetNPatchMode");
		return _dev->SetNPatchMode(nSegments);
	}
	STDMETHOD_(float, GetNPatchMode)(THIS) {
		D3D_CALL_LOG("d3d GetNPatchMode");
		return _dev->GetNPatchMode();
	}
	STDMETHOD(DrawPrimitive)(THIS_ D3DPRIMITIVETYPE PrimitiveType,UINT StartVertex,UINT PrimitiveCount) {
		D3D_CALL_LOG("d3d DrawPrimitive");
		return _dev->DrawPrimitive(PrimitiveType,StartVertex,PrimitiveCount);
	}
	STDMETHOD(DrawIndexedPrimitive)(THIS_ D3DPRIMITIVETYPE PrimitiveType,INT BaseVertexIndex,UINT MinVertexIndex,UINT NumVertices,UINT startIndex,UINT primCount) {
		D3D_CALL_LOG("d3d DrawIndexedPrimitive");
		if (_hookRenderState.isDIPActive()) {
			return _dev->DrawIndexedPrimitive(PrimitiveType,BaseVertexIndex,MinVertexIndex,NumVertices,startIndex,primCount);
		}
		_hookRenderState.setDIPActive(true);

		DWORD abEnabled = 0;
		long selStage = _hookRenderState.selectedTextureStage();
		if (selStage >= 0) {
			_dev->GetRenderState(D3DRS_ALPHABLENDENABLE, &abEnabled);
			//_dev->GetRenderState(D3DRS_AMBIENT, &cAmbient);

			_dev->SetTexture(selStage, _hookRenderState.getSelectionTexture());
		}

		if (Interop::OK() && _hookRenderState.isSnapping()) {
			MM_LOG_INFO("Snap started");
			MM_LOG_INFO("active streams:");
			for (int i = 0; i < MaxActiveStreams; ++i) {
				if (activeStreams[i]) {
					MM_LOG_INFO(fmt::format("   {}", i));
					if (i > 0) {
						MM_LOG_INFO("   Warning: this stream may not be supported");
					}
				}
			}
			DWORD blendingEnabled = 0;
			HRESULT hr = 0;
			hr = _dev->GetRenderState(D3DRS_INDEXEDVERTEXBLENDENABLE, &blendingEnabled);
			if (SUCCEEDED(hr) && blendingEnabled)
				MM_LOG_INFO("   WARNING! vertex blending is enabled");

			SnapshotData sd;
			sd.primType = PrimitiveType;
			sd.baseVertexIndex = BaseVertexIndex;
			sd.minVertexIndex = MinVertexIndex;
			sd.numVertices = NumVertices;
			sd.startIndex = startIndex;
			sd.primCount = primCount;

			bool ok = false;
			// sharpdx doesn't expose this, so need to grab it here
			hr = _dev->GetVertexDeclaration(&(sd.decl));
			ok = SUCCEEDED(hr);
			if (!ok) {
				MM_LOG_INFO(fmt::format("Error, can't get vertex declaration.  Cannot snap; HR: {:x}", hr));
			}

			hr = _dev->GetIndices(&sd.ib);
			ok = SUCCEEDED(hr);
			if (!ok) {
				MM_LOG_INFO(fmt::format("Error, can't get index buffer.  Cannot snap; HR: {:x}", hr));
			}

			if (ok) {
				Interop::Callbacks().TakeSnapshot(_dev, &sd); // note, the real device is passed to managed code, not the hook device.
			}

			SAFE_RELEASE(sd.decl);
			SAFE_RELEASE(sd.ib);
		}

		bool drawInput = true;
		NativeModData* mod = NULL;
		if (_hookRenderState.getShowModMesh() && ((mod = _hookRenderState.findMod(NumVertices, primCount)) != NULL)) {
			if (mod->decl && mod->vb) {
				_hookRenderState.saveRenderState(this);
				_dev->SetVertexDeclaration(mod->decl);
				_dev->SetStreamSource(0, mod->vb, 0, mod->modData.vertSizeBytes);
				_dev->SetIndices(NULL);
				for (Uint32 i = 0; i < MaxModTextures; ++i) {
					if (mod->texture[i]) {
						_dev->SetTexture(i, mod->texture[i]);
					}
				}
				_dev->DrawPrimitive((D3DPRIMITIVETYPE)mod->modData.primType, 0, mod->modData.primCount);
				_hookRenderState.restoreRenderState(this);
			}

			if (mod->modData.modType == CPUReplacement || mod->modData.modType == GPUReplacement || mod->modData.modType == Deletion) {
				drawInput = false;
			}
		}

		HRESULT hr = S_OK;
		if (drawInput) {
			hr = _dev->DrawIndexedPrimitive(PrimitiveType,BaseVertexIndex,MinVertexIndex,NumVertices,startIndex,primCount);
		}

		if (_hookRenderState.selectedTextureStage() >= 0) {
			_dev->SetRenderState(D3DRS_ALPHABLENDENABLE, abEnabled );

			_dev->SetTexture(selStage, _hookRenderState.currentTexture());
		}

		_hookRenderState.setDIPActive(false);

		return hr;
	}
	STDMETHOD(DrawPrimitiveUP)(THIS_ D3DPRIMITIVETYPE PrimitiveType,UINT PrimitiveCount,CONST void* pVertexStreamZeroData,UINT VertexStreamZeroStride) {
		D3D_CALL_LOG("d3d DrawPrimitiveUP");
		return _dev->DrawPrimitiveUP(PrimitiveType,PrimitiveCount,pVertexStreamZeroData,VertexStreamZeroStride);
	}
	STDMETHOD(DrawIndexedPrimitiveUP)(THIS_ D3DPRIMITIVETYPE PrimitiveType,UINT MinVertexIndex,UINT NumVertices,UINT PrimitiveCount,CONST void* pIndexData,D3DFORMAT IndexDataFormat,CONST void* pVertexStreamZeroData,UINT VertexStreamZeroStride) {
		D3D_CALL_LOG("d3d DrawIndexedPrimitiveUP");
		return _dev->DrawIndexedPrimitiveUP(PrimitiveType,MinVertexIndex,NumVertices,PrimitiveCount,pIndexData,IndexDataFormat,pVertexStreamZeroData,VertexStreamZeroStride);
	}
	STDMETHOD(ProcessVertices)(THIS_ UINT SrcStartIndex,UINT DestIndex,UINT VertexCount,IDirect3DVertexBuffer9* pDestBuffer,IDirect3DVertexDeclaration9* pVertexDecl,DWORD Flags) {
		D3D_CALL_LOG("d3d ProcessVertices");
		IDirect3DVertexBuffer9* realVB = pDestBuffer;

		if (MM_HOOK_VERTEX_BUFFERS) {
			Hook_IDirect3DVertexBuffer9* hvb = (Hook_IDirect3DVertexBuffer9*)pDestBuffer;
			realVB = hvb->vb();
		}

		return _dev->ProcessVertices(SrcStartIndex,DestIndex,VertexCount,realVB,pVertexDecl,Flags);
	}
	STDMETHOD(CreateVertexDeclaration)(THIS_ CONST D3DVERTEXELEMENT9* pVertexElements,IDirect3DVertexDeclaration9** ppDecl) {
		D3D_CALL_LOG("d3d CreateVertexDeclaration");
		return _dev->CreateVertexDeclaration(pVertexElements,ppDecl);
	}
	STDMETHOD(SetVertexDeclaration)(THIS_ IDirect3DVertexDeclaration9* pDecl) {
		D3D_CALL_LOG("d3d SetVertexDeclaration");
		_hookRenderState._lastDecl = pDecl;
		_hookRenderState._lastWasFVF = false;

		return _dev->SetVertexDeclaration(pDecl);
	}
	STDMETHOD(GetVertexDeclaration)(THIS_ IDirect3DVertexDeclaration9** ppDecl) {
		D3D_CALL_LOG("d3d GetVertexDeclaration");
		return _dev->GetVertexDeclaration(ppDecl);
	}
	STDMETHOD(SetFVF)(THIS_ DWORD FVF) {
		D3D_CALL_LOG("d3d SetFVF");
		_hookRenderState._lastFVF = FVF;
		_hookRenderState._lastWasFVF = true;

		return _dev->SetFVF(FVF);
	}
	STDMETHOD(GetFVF)(THIS_ DWORD* pFVF) {
		D3D_CALL_LOG("d3d GetFVF");
		return _dev->GetFVF(pFVF);
	}
	STDMETHOD(CreateVertexShader)(THIS_ CONST DWORD* pFunction,IDirect3DVertexShader9** ppShader) {
		D3D_CALL_LOG("d3d CreateVertexShader");
		return _dev->CreateVertexShader(pFunction,ppShader);
	}
	
	STDMETHOD(SetVertexShader)(THIS_ IDirect3DVertexShader9* pShader) {
		D3D_CALL_LOG("d3d SetVertexShader");
		return _dev->SetVertexShader(pShader);
	}
	STDMETHOD(GetVertexShader)(THIS_ IDirect3DVertexShader9** ppShader) {
		D3D_CALL_LOG("d3d GetVertexShader");
		return _dev->GetVertexShader(ppShader);
	}

	float _checkShaderConstantBuf[32000];
	vector<UINT> _invalidConstants;
		
	void _checkShaderConstants(IDirect3DDevice9* dev, UINT StartRegister, FloatConstantMap& vsFloatConstants) {
#if TRACK_SHADER_CONSTANTS
		_invalidConstants.clear();

		for (FloatConstantMap::iterator iter = vsFloatConstants.begin(); 
			iter != vsFloatConstants.end();
			++iter) {
				// need to check all constants, not just the ones after StartRegister.  This is because setting StartRegister
				// may have invalidated earlier constants whose size caused them to overlap.  
				// TODO(profile) what is the cost of doing these checks? may need more optimization

				// check what the device thinks this value is currently
				UINT sCount = iter->second.getCount();
				if (sCount > sizeof(_checkShaderConstantBuf)) {
					MM_LOG_INFO(fmt::format("WARNING: unable to check shader constant {} size, too large: {}", StartRegister, sCount));
					continue;
				}
				HRESULT hr = _dev->GetVertexShaderConstantF(iter->first, _checkShaderConstantBuf, sCount);

				float* sConstantData = iter->second.getData();

				size_t compared = sCount*sizeof(float)*4;
				int cmp = memcmp((Uint8*)_checkShaderConstantBuf, (Uint8*)sConstantData, compared);

				if (cmp != 0) {
					// this register has been invalidated by a set
					_invalidConstants.push_back(iter->first);
				}
				//if (cmp != 0 && StartRegister == iter->first) {
				//	MM_LOG_INFO("vc DOH, mismatch on this register");
				//}
				//if (cmp != 0) {
				//	MM_LOG_INFO(format("vc FAIL on %d, compared %d bytes, class %08X/%d, dp %08X") % iter->first % compared % (DWORD)&(iter->second) 
				//		% sizeof(iter->second) % (DWORD)sConstantData);
				//}  else {
				//	MM_LOG_INFO(format("vc match on %d, compared %d bytes, class %08X/%d, dp %08X") % iter->first % compared % (DWORD)&(iter->second) 
				//		% sizeof(iter->second) % (DWORD)sConstantData);
				//}
		}

		for (vector<UINT>::iterator iter = _invalidConstants.begin();
			iter != _invalidConstants.end();
			++iter) {
				vsFloatConstants[*iter].clear();
				vsFloatConstants.erase(*iter);
		}
#endif
	}

	STDMETHOD(SetVertexShaderConstantF)(THIS_ UINT StartRegister,CONST float* pConstantData,UINT Vector4fCount) {
		D3D_CALL_LOG("d3d SetVertexShaderConstantF");

		HRESULT hr = _dev->SetVertexShaderConstantF(StartRegister,pConstantData,Vector4fCount);
#if TRACK_SHADER_CONSTANTS
		if (SUCCEEDED(hr)) {
			_hookRenderState.vsFloatConstants[StartRegister].set(pConstantData,Vector4fCount);
			_checkShaderConstants(this,StartRegister,_hookRenderState.vsFloatConstants);
		} else {
			_hookRenderState.vsFloatConstants[StartRegister].clear();
			_hookRenderState.vsFloatConstants.erase(StartRegister);
		}
#endif
		return hr;

	}
	STDMETHOD(GetVertexShaderConstantF)(THIS_ UINT StartRegister,float* pConstantData,UINT Vector4fCount) {
		D3D_CALL_LOG("d3d GetVertexShaderConstantF");
		return _dev->GetVertexShaderConstantF(StartRegister,pConstantData,Vector4fCount);
	}
	STDMETHOD(SetVertexShaderConstantI)(THIS_ UINT StartRegister,CONST int* pConstantData,UINT Vector4iCount) {
		D3D_CALL_LOG("d3d SetVertexShaderConstantI");
#if TRACK_SHADER_CONSTANTS
		_hookRenderState.vsIntConstants[StartRegister].set(pConstantData,Vector4iCount);
#endif
		return _dev->SetVertexShaderConstantI(StartRegister,pConstantData,Vector4iCount);
	}
	STDMETHOD(GetVertexShaderConstantI)(THIS_ UINT StartRegister,int* pConstantData,UINT Vector4iCount) {
		D3D_CALL_LOG("d3d GetVertexShaderConstantI");
		return _dev->GetVertexShaderConstantI(StartRegister,pConstantData,Vector4iCount);
	}
	STDMETHOD(SetVertexShaderConstantB)(THIS_ UINT StartRegister,CONST BOOL* pConstantData,UINT  BoolCount) {
		D3D_CALL_LOG("d3d SetVertexShaderConstantB");
#if TRACK_SHADER_CONSTANTS
		_hookRenderState.vsBoolConstants[StartRegister].set(pConstantData,BoolCount);
#endif

		return _dev->SetVertexShaderConstantB(StartRegister,pConstantData,BoolCount);
	}
	STDMETHOD(GetVertexShaderConstantB)(THIS_ UINT StartRegister,BOOL* pConstantData,UINT BoolCount) {
		D3D_CALL_LOG("d3d GetVertexShaderConstantB");
		return _dev->GetVertexShaderConstantB(StartRegister,pConstantData,BoolCount);
	}

	STDMETHOD(SetStreamSource)(THIS_ UINT StreamNumber,IDirect3DVertexBuffer9* pStreamData,UINT OffsetInBytes,UINT Stride) {
		D3D_CALL_LOG("d3d SetStreamSource");
		IDirect3DVertexBuffer9* realVB = pStreamData;

		if (StreamNumber >= MaxActiveStreams) {
			MM_LOG_INFO(fmt::format("****************************** Warning, stream out of range: {}", StreamNumber));
		} else {
			activeStreams[StreamNumber] = pStreamData != NULL;
		}
		if (StreamNumber == 0) {
			_currStream0VB = pStreamData;
		}

		//if (dynamic_cast<Hook_IDirect3DVertexBuffer9*>(pStreamData)) {
		if (pStreamData && MM_HOOK_VERTEX_BUFFERS) {
			realVB = ((Hook_IDirect3DVertexBuffer9*)pStreamData)->vb();
		} 

		if (StreamNumber == 0 && MM_HOOK_VERTEX_BUFFERS) {
			_hookRenderState._currHookVB0 = (Hook_IDirect3DVertexBuffer9*)pStreamData;
		}

		return _dev->SetStreamSource(StreamNumber,realVB,OffsetInBytes,Stride);
	}
	STDMETHOD(GetStreamSource)(THIS_ UINT StreamNumber,IDirect3DVertexBuffer9** ppStreamData,UINT* pOffsetInBytes,UINT* pStride) {
		D3D_CALL_LOG("d3d GetStreamSource");
		return _dev->GetStreamSource(StreamNumber,ppStreamData,pOffsetInBytes,pStride);
	}
	STDMETHOD(SetStreamSourceFreq)(THIS_ UINT StreamNumber,UINT Setting) {
		D3D_CALL_LOG("d3d SetStreamSourceFreq");
		return _dev->SetStreamSourceFreq(StreamNumber,Setting);
	}
	STDMETHOD(GetStreamSourceFreq)(THIS_ UINT StreamNumber,UINT* pSetting) {
		D3D_CALL_LOG("d3d GetStreamSourceFreq");
		return _dev->GetStreamSourceFreq(StreamNumber,pSetting);
	}
	STDMETHOD(SetIndices)(THIS_ IDirect3DIndexBuffer9* pIndexData) {
		D3D_CALL_LOG("d3d SetIndices");
		_currIndices = pIndexData;
		return _dev->SetIndices(pIndexData);
	}
	STDMETHOD(GetIndices)(THIS_ IDirect3DIndexBuffer9** ppIndexData) {
		D3D_CALL_LOG("d3d GetIndices");
		return _dev->GetIndices(ppIndexData);
	}
	STDMETHOD(CreatePixelShader)(THIS_ CONST DWORD* pFunction,IDirect3DPixelShader9** ppShader) {
		D3D_CALL_LOG("d3d CreatePixelShader");
		return _dev->CreatePixelShader(pFunction,ppShader);
	}
	STDMETHOD(SetPixelShader)(THIS_ IDirect3DPixelShader9* pShader) {
		D3D_CALL_LOG("d3d SetPixelShader");
		return _dev->SetPixelShader(pShader);
	}
	STDMETHOD(GetPixelShader)(THIS_ IDirect3DPixelShader9** ppShader) {
		D3D_CALL_LOG("d3d GetPixelShader");
		return _dev->GetPixelShader(ppShader);
	}
	STDMETHOD(SetPixelShaderConstantF)(THIS_ UINT StartRegister,CONST float* pConstantData,UINT Vector4fCount) {
		D3D_CALL_LOG("d3d SetPixelShaderConstantF");
		
		HRESULT hr = _dev->SetPixelShaderConstantF(StartRegister,pConstantData,Vector4fCount);

#if TRACK_SHADER_CONSTANTS
		if (SUCCEEDED(hr)) {
			_hookRenderState.psFloatConstants[StartRegister].set(pConstantData,Vector4fCount);
			_checkShaderConstants(this,StartRegister,_hookRenderState.psFloatConstants);
		} else {
			_hookRenderState.psFloatConstants[StartRegister].clear();
			_hookRenderState.psFloatConstants.erase(StartRegister);
		}
#endif
		return hr;
		
	}
	STDMETHOD(GetPixelShaderConstantF)(THIS_ UINT StartRegister,float* pConstantData,UINT Vector4fCount) {
		D3D_CALL_LOG("d3d GetPixelShaderConstantF");
		return _dev->GetPixelShaderConstantF(StartRegister,pConstantData,Vector4fCount);
	}
	STDMETHOD(SetPixelShaderConstantI)(THIS_ UINT StartRegister,CONST int* pConstantData,UINT Vector4iCount) {
		D3D_CALL_LOG("d3d SetPixelShaderConstantI");
#if TRACK_SHADER_CONSTANTS
		_hookRenderState.psIntConstants[StartRegister].set(pConstantData,Vector4iCount);
#endif

		return _dev->SetPixelShaderConstantI(StartRegister,pConstantData,Vector4iCount);
	}
	STDMETHOD(GetPixelShaderConstantI)(THIS_ UINT StartRegister,int* pConstantData,UINT Vector4iCount) {
		D3D_CALL_LOG("d3d GetPixelShaderConstantI");
		return _dev->GetPixelShaderConstantI(StartRegister,pConstantData,Vector4iCount);
	}
	STDMETHOD(SetPixelShaderConstantB)(THIS_ UINT StartRegister,CONST BOOL* pConstantData,UINT  BoolCount) {
		D3D_CALL_LOG("d3d SetPixelShaderConstantB");
#if TRACK_SHADER_CONSTANTS
		_hookRenderState.psBoolConstants[StartRegister].set(pConstantData,BoolCount);
#endif
		return _dev->SetPixelShaderConstantB(StartRegister,pConstantData,BoolCount);
	}
	STDMETHOD(GetPixelShaderConstantB)(THIS_ UINT StartRegister,BOOL* pConstantData,UINT BoolCount) {
		D3D_CALL_LOG("d3d GetPixelShaderConstantB");
		return _dev->GetPixelShaderConstantB(StartRegister,pConstantData,BoolCount);
	}
	STDMETHOD(DrawRectPatch)(THIS_ UINT Handle,CONST float* pNumSegs,CONST D3DRECTPATCH_INFO* pRectPatchInfo) {
		D3D_CALL_LOG("d3d DrawRectPatch");
		return _dev->DrawRectPatch(Handle,pNumSegs,pRectPatchInfo);
	}
	STDMETHOD(DrawTriPatch)(THIS_ UINT Handle,CONST float* pNumSegs,CONST D3DTRIPATCH_INFO* pTriPatchInfo) {
		D3D_CALL_LOG("d3d DrawTriPatch");
		return _dev->DrawTriPatch(Handle,pNumSegs,pTriPatchInfo);
	}
	STDMETHOD(DeletePatch)(THIS_ UINT Handle) {
		D3D_CALL_LOG("d3d DeletePatch");
		return _dev->DeletePatch(Handle);
	}
	STDMETHOD(CreateQuery)(THIS_ D3DQUERYTYPE Type,IDirect3DQuery9** ppQuery) {
		D3D_CALL_LOG("d3d CreateQuery");
		return _dev->CreateQuery(Type,ppQuery);
	}
};


