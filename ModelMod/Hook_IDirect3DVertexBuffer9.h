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

class Hook_IDirect3DVertexBuffer9 : public IDirect3DVertexBuffer9
{
	IDirect3DVertexBuffer9 *_vb;
	int _refCount;
	UINT _size;
	BYTE* _data;
	UINT _lockOffset;
	UINT _lockSize;
	void* _lockData;
public:
	Hook_IDirect3DVertexBuffer9(IDirect3DVertexBuffer9* vb, UINT size) {
		_vb = vb;
		_refCount = 0;
		_size = size;
		_data = NULL;
		_lockOffset = 0;
		_lockSize = 0;
		_lockData = NULL;
	}
	virtual ~Hook_IDirect3DVertexBuffer9(void) {
		delete [] _data;
	}

	IDirect3DVertexBuffer9* vb() { return _vb; }
	BYTE* data() { return _data; }

    /*** IUnknown methods ***/
    STDMETHOD(QueryInterface)(THIS_ REFIID riid, void** ppvObj) {
		return _vb->QueryInterface(riid,ppvObj);
	}
    STDMETHOD_(ULONG,AddRef)(THIS) {
		_refCount++;

		return _vb->AddRef();
	}
    STDMETHOD_(ULONG,Release)(THIS) {
		_refCount--;
		HRESULT hr = _vb->Release();
		if (_refCount <= 0) {
			delete this;
		}
		return hr;
	}

    /*** IDirect3DResource9 methods ***/
    STDMETHOD(GetDevice)(THIS_ IDirect3DDevice9** ppDevice) {
		return _vb->GetDevice(ppDevice);
	}
    STDMETHOD(SetPrivateData)(THIS_ REFGUID refguid,CONST void* pData,DWORD SizeOfData,DWORD Flags) {
		return _vb->SetPrivateData(refguid,pData,SizeOfData,Flags);
	}
    STDMETHOD(GetPrivateData)(THIS_ REFGUID refguid,void* pData,DWORD* pSizeOfData) {
		return _vb->GetPrivateData(refguid,pData,pSizeOfData);
	}
    STDMETHOD(FreePrivateData)(THIS_ REFGUID refguid) {
		return _vb->FreePrivateData(refguid);
	}
    STDMETHOD_(DWORD, SetPriority)(THIS_ DWORD PriorityNew) {
		return _vb->SetPriority(PriorityNew);
	}
    STDMETHOD_(DWORD, GetPriority)(THIS) {
		return _vb->GetPriority();
	}
    STDMETHOD_(void, PreLoad)(THIS) {
		return _vb->PreLoad();
	}
    STDMETHOD_(D3DRESOURCETYPE, GetType)(THIS) {
		return _vb->GetType();
	}
    STDMETHOD(Lock)(THIS_ UINT OffsetToLock,UINT SizeToLock,void** ppbData,DWORD Flags) {
		_lockData = NULL;
		HRESULT hr = _vb->Lock(OffsetToLock,SizeToLock,ppbData,Flags);
		if (SUCCEEDED(hr)) {
			_lockData = *ppbData;
			_lockOffset = OffsetToLock;
			_lockSize = SizeToLock;
		}
		return hr;
	}
    STDMETHOD(Unlock)(THIS) {
		if (_lockData) {
			if (!_data) {
				_data = new BYTE[_size];
			}
			// copy the lock region into data
			UINT end = _lockSize;
			if (end == 0) {
				end = _size;
			}
			memcpy(_data + _lockOffset, (BYTE*)_lockData + _lockOffset, end);
		}
		return _vb->Unlock();
	}
    STDMETHOD(GetDesc)(THIS_ D3DVERTEXBUFFER_DESC *pDesc) {
		return _vb->GetDesc(pDesc);
	}
};

