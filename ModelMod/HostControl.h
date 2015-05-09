#include <metahost.h>

// see MMAppDomain.fs for info on how to regenerate this.  It must be in-sync with the managed definition.
// Bah #import means can't use /MP (multi-processor compilation).  oh well.
#import "..\MMAppDomain\MMAppDomain.tlb" 
using namespace ModelModCLRAppDomain;

class HostControl : public IHostControl {
	IMMAppDomainMananger* _manager;
	LONG _cref;
public:
	HostControl() {
		_manager = NULL;
		_cref = 0;
	}

	virtual ~HostControl() {
	}

	IMMAppDomainMananger* GetDomainMananger() {
		return _manager;
	}

	// COM gunk:
	// https://msdn.microsoft.com/en-us/library/office/cc839627%28v=office.15%29.aspx
	virtual HRESULT STDMETHODCALLTYPE QueryInterface (REFIID   riid,
										   LPVOID * ppvObj)
	{
		// Always set out parameter to NULL, validating it first.
		if (!ppvObj)
			return E_INVALIDARG;
		*ppvObj = NULL;
		if (riid == IID_IUnknown)
		{
			// Increment the reference count and return the pointer.
			*ppvObj = (LPVOID)this;
			AddRef();
			return NOERROR;
		}
		return E_NOINTERFACE;
	}
	virtual ULONG STDMETHODCALLTYPE AddRef()
	{
		InterlockedIncrement(&_cref);
		return _cref;
	}
	virtual ULONG STDMETHODCALLTYPE Release()
	{
		// Decrement the object's internal counter.
		ULONG ulRefCount = InterlockedDecrement(&_cref);
		// don't want this to bite me in the ass.  this object is a singleton.
		//if (0 == _cref)
		//{
		//	delete this;
		//}
		return ulRefCount;
	}

	virtual HRESULT STDMETHODCALLTYPE GetHostManager( 
    /* [in] */ REFIID riid,
    /* [out] */ void **ppObject) {
		return E_NOINTERFACE; 
	}
        
    virtual HRESULT STDMETHODCALLTYPE SetAppDomainManager( 
        /* [in] */ DWORD dwAppDomainID,
        /* [in] */ IUnknown *pUnkAppDomainManager) {
      HRESULT hr = E_FAIL;
      hr = pUnkAppDomainManager->QueryInterface(__uuidof(IMMAppDomainMananger), (void**) &_manager);
      return hr;
	}
};