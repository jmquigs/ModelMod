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

#define MM_HOOK_VERTEX_BUFFERS 0

struct IUnknown;
struct IDirect3DDevice9;

#ifndef SAFE_RELEASE
#define SAFE_RELEASE(s) {if (s) { s->Release(); s = NULL; }}
#endif

#include <string>

namespace ModelMod {

typedef unsigned char Uint8;
typedef unsigned short Uint16;
typedef unsigned int Uint32;

// Be sure to add new types to GetType as well as here
enum ModType {
	None,
	CPUAdditive,
	CPUReplacement,
	GPUReplacement,
	GPUPertubation,
	Deletion
};

// TODO: can these be removed? does native code need them?
ModType GetType(std::string sType);
std::string GetTypeString(ModType type);

// ---------------------------------------------------------------------------
// ConstantData
template <class T, Uint32 elCount> class ConstantData {
	T* _data;
	T* _buffer;
	Uint32 _bufSize;
	Uint32 _count;

public:
	ConstantData() {
		_data = NULL;
		_buffer = NULL;
		_bufSize = 0;
		_count = 0;
	}
	~ConstantData() {
		// TODO(leak): if you pass a copy of a map of these for instance to a function
		// all the pointers will get deleted when that map goes out of scope.  
		// so never delete them here.
		// there is probably some c++-11 way of handling this, but anyway this code is unused ATM.
		//delete [] _buffer;
	}

	void clear() {
		delete [] _buffer;
		_data = _buffer = NULL;
		_bufSize = _count = 0;
	}

	void set(const T* data, Uint32 count) {
		UINT requiredSize = count * elCount;
		if (requiredSize > _bufSize) {
			_bufSize = requiredSize;
			delete [] _buffer;
			_buffer = new T[requiredSize];
		}
		_count = count;
		if (data) {
			memcpy(_buffer,data,sizeof(T)*(requiredSize));
			_data = _buffer;
		} else {
			_data = NULL;
		}
	}

	T* getData() { return _data; }
	Uint32 getCount() { return _count; }
};

// ---------------------------------------------------------------------------
// internal interfaces
class ID3DResourceTracker {
public:
	virtual void add(IUnknown* res) = 0;
	virtual void release(IUnknown* res) = 0;
};

class ISceneNotify {
public:
	virtual void onBeginScene() {}
	virtual void onEndScene() {}
};

class IRenderState {
public:
	virtual void saveRenderState(IDirect3DDevice9* dev) = 0;
	virtual void restoreRenderState(IDirect3DDevice9* dev) = 0;

	virtual void addSceneNotify(ISceneNotify* notify) = 0;
};

};