// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

#pragma once

#include <d3d9.h>

#include "Hook_IDirect3DDevice9.h"
#include "Log.h"
using namespace ModelMod;

/// Wraps IDirect3D9.  Primary purpose is to return the hook device when the device is created.
class Hook_IDirect3D9 : public IDirect3D9 {
private:
	IDirect3D9* _d3d9;
	Hook_IDirect3DDevice9* _hDev;

	static const string LogCategory;

public:
	Hook_IDirect3D9(IDirect3D9* d3d9) :
		_d3d9(d3d9) ,
		_hDev(NULL) {
	}

	virtual ~Hook_IDirect3D9(void) {
	}

	/*** IUnknown methods ***/
	STDMETHOD(QueryInterface)(THIS_ REFIID riid, void** ppvObj) {
		MM_LOG_INFO("QueryInterface");
		return _d3d9->QueryInterface(riid,ppvObj);
	}
	STDMETHOD_(ULONG,AddRef)(THIS) {
		return _d3d9->AddRef();
	}
	STDMETHOD_(ULONG,Release)(THIS) {
		return _d3d9->Release();
	}

	/*** IDirect3D9 methods ***/
	STDMETHOD(RegisterSoftwareDevice)(THIS_ void* pInitializeFunction) {
		return _d3d9->RegisterSoftwareDevice(pInitializeFunction);
	}
	STDMETHOD_(UINT, GetAdapterCount)(THIS) {
		return _d3d9->GetAdapterCount();
	}
	STDMETHOD(GetAdapterIdentifier)(THIS_ UINT Adapter,DWORD Flags,D3DADAPTER_IDENTIFIER9* pIdentifier) {
		return _d3d9->GetAdapterIdentifier(Adapter,Flags,pIdentifier);
	}
	STDMETHOD_(UINT, GetAdapterModeCount)(THIS_ UINT Adapter,D3DFORMAT Format) {
		return _d3d9->GetAdapterModeCount(Adapter,Format);
	}
	STDMETHOD(EnumAdapterModes)(THIS_ UINT Adapter,D3DFORMAT Format,UINT Mode,D3DDISPLAYMODE* pMode) {
		return _d3d9->EnumAdapterModes(Adapter,Format,Mode,pMode);
	}
	STDMETHOD(GetAdapterDisplayMode)(THIS_ UINT Adapter,D3DDISPLAYMODE* pMode) {
		return _d3d9->GetAdapterDisplayMode(Adapter,pMode);
	}
	STDMETHOD(CheckDeviceType)(THIS_ UINT Adapter,D3DDEVTYPE DevType,D3DFORMAT AdapterFormat,D3DFORMAT BackBufferFormat,BOOL bWindowed) {
		return _d3d9->CheckDeviceType(Adapter,DevType,AdapterFormat,BackBufferFormat,bWindowed);
	}
	STDMETHOD(CheckDeviceFormat)(THIS_ UINT Adapter,D3DDEVTYPE DeviceType,D3DFORMAT AdapterFormat,DWORD Usage,D3DRESOURCETYPE RType,D3DFORMAT CheckFormat) {
		return _d3d9->CheckDeviceFormat(Adapter,DeviceType,AdapterFormat,Usage,RType,CheckFormat);
	}
	STDMETHOD(CheckDeviceMultiSampleType)(THIS_ UINT Adapter,D3DDEVTYPE DeviceType,D3DFORMAT SurfaceFormat,BOOL Windowed,D3DMULTISAMPLE_TYPE MultiSampleType,DWORD* pQualityLevels) {
		return _d3d9->CheckDeviceMultiSampleType(Adapter,DeviceType,SurfaceFormat,Windowed,MultiSampleType,pQualityLevels);
	}
	STDMETHOD(CheckDepthStencilMatch)(THIS_ UINT Adapter,D3DDEVTYPE DeviceType,D3DFORMAT AdapterFormat,D3DFORMAT RenderTargetFormat,D3DFORMAT DepthStencilFormat) {
		return _d3d9->CheckDepthStencilMatch(Adapter,DeviceType,AdapterFormat,RenderTargetFormat,DepthStencilFormat);
	}
	STDMETHOD(CheckDeviceFormatConversion)(THIS_ UINT Adapter,D3DDEVTYPE DeviceType,D3DFORMAT SourceFormat,D3DFORMAT TargetFormat) {
		return _d3d9->CheckDeviceFormatConversion(Adapter,DeviceType,SourceFormat,TargetFormat);
	}
	STDMETHOD(GetDeviceCaps)(THIS_ UINT Adapter,D3DDEVTYPE DeviceType,D3DCAPS9* pCaps) {
		return _d3d9->GetDeviceCaps(Adapter,DeviceType,pCaps);
	}
	STDMETHOD_(HMONITOR, GetAdapterMonitor)(THIS_ UINT Adapter) {
		return _d3d9->GetAdapterMonitor(Adapter);
	}
	STDMETHOD(CreateDevice)(THIS_ UINT Adapter,D3DDEVTYPE DeviceType,HWND hFocusWindow,DWORD BehaviorFlags,D3DPRESENT_PARAMETERS* pPresentationParameters,IDirect3DDevice9** ppReturnedDeviceInterface) {
		MM_LOG_INFO("CreateDevice");

		if (BehaviorFlags & D3DCREATE_PUREDEVICE) {
			MM_LOG_INFO("WARNING: pure device in use, state retrieval may not work");
			//BehaviorFlags = BehaviorFlags & ~D3DCREATE_PUREDEVICE;
		}
		//int mixed = BehaviorFlags & D3DCREATE_MIXED_VERTEXPROCESSING;
		//int hw = BehaviorFlags & D3DCREATE_HARDWARE_VERTEXPROCESSING;
		//int sw = BehaviorFlags & D3DCREATE_SOFTWARE_VERTEXPROCESSING;
		HRESULT hr = _d3d9->CreateDevice(Adapter,DeviceType,hFocusWindow,BehaviorFlags,pPresentationParameters,ppReturnedDeviceInterface);
		if (SUCCEEDED(hr)) {
			if (_hDev) {
				MM_LOG_INFO("Replacing Device");
				delete _hDev;
			}
			_hDev = new Hook_IDirect3DDevice9(this, *ppReturnedDeviceInterface);
			*ppReturnedDeviceInterface = _hDev;
		} else {
			MM_LOG_INFO("Create Device FAILED");
		}
		return hr;
	}
};

