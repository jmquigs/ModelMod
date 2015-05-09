#pragma once

#define MM_HOOK_VERTEX_BUFFERS 0

struct IUnknown;
struct IDirect3DDevice9;

#ifdef MODELMOD_DO_EXPORT
#  define MODELMOD_EXPORT __declspec(dllexport)
#else
#  define MODELMOD_EXPORT __declspec(dllimport)
#endif

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
		// TODO(leak): this is seriously bad...if you pass a copy of a map of these for instance to a function
		// all the pointers will get deleted when that map goes out of scope.
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