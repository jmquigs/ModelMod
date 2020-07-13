pub use winapi::shared::d3d9::*;
pub use winapi::shared::d3d9types::*;
pub use winapi::shared::minwindef::*;
pub use winapi::um::winnt::{HRESULT, LPCWSTR};

use hookd3d9::dev_state;

struct ConstantData<T> {
    pub start_reg:UINT,
    pub data: Vec<T>,
    pub count: usize,
    pub prims_per_el: usize
}

impl<T> ConstantData<T> 
where T: Default + Copy
{
    pub fn new(prims_per_el:usize) -> Self {
        Self {
            start_reg: 0,
            data: Vec::new(),
            count: 0,
            prims_per_el: prims_per_el
        }
    }
    
    pub fn set(&mut self, StartRegister: UINT,
        pConstantData: *const T,
        Vector4fCount: UINT) {
            let Vector4fCount:usize = Vector4fCount as usize;// Vector4fCount.try_into().unwrap();
            
            // set size.  when we truncate the vector should preserve its existing buffer 
            // so that we don't keep reallocating (though, it will call drop on the excess, 
            // hopefully that is fast for primitive types)
            let curr_el_count = self.data.len() / self.prims_per_el;
            if Vector4fCount > curr_el_count {
                self.data.resize_with(Vector4fCount * self.prims_per_el, || Default::default());
            } else if Vector4fCount < curr_el_count {
                self.data.truncate(Vector4fCount * self.prims_per_el)
            }
            self.count = Vector4fCount;
            self.start_reg = StartRegister;
            let num_prims = self.count * self.prims_per_el;
            
            // TODO: should just "memcpy"
            // safety note: no way to verify that pConstantData actually points to the number 
            // of elements we want.  if it doesn't, this will read invalid memory.
            for el in 0..num_prims {
                self.data[el] = unsafe { *pConstantData.offset(el as isize) };
            }
        }
}

type FloatConstantData = ConstantData<f32>;
type BoolConstantData = ConstantData<BOOL>;
type IntConstantData = ConstantData<i32>;

pub fn is_enabled() -> bool {
    true
}

pub unsafe extern "system" fn hook_set_vertex_sc_f(
    THIS: *mut IDirect3DDevice9,
    StartRegister: UINT,
    pConstantData: *const f32,
    Vector4fCount: UINT
) -> HRESULT {
    (dev_state().hook_direct3d9device.as_ref().unwrap().real_set_vertex_sc_f)(THIS, StartRegister, pConstantData, Vector4fCount)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check<T>(cd:&mut ConstantData<T>, start_reg:UINT, count:UINT, floats:&Vec<T>) 
    where T: Default + Copy + std::fmt::Debug + PartialEq {
        let constant_data: *const T = floats.as_ptr();
        cd.set(start_reg,constant_data, count);
        assert_eq!(cd.start_reg, start_reg);
        assert_eq!(cd.data, *floats);
        assert_eq!(cd.count, count as usize);    
    }
    
    #[test]
    pub fn test_bool() {
        let mut cd:BoolConstantData = BoolConstantData::new(1);
        check(&mut cd, 1, 1, &vec![FALSE]);
        check(&mut cd, 3, 3, &vec![TRUE, FALSE, TRUE]);
        check(&mut cd, 5, 2, &vec![FALSE, FALSE]);
    }
    #[test]
    pub fn test_int() {
        let mut cd:IntConstantData = IntConstantData::new(1);
        check(&mut cd, 1, 1, &vec![6]);
        check(&mut cd, 1, 3, &vec![6, 6, 6]);
        check(&mut cd, 6, 2, &vec![6, 6]);
    }
    
    #[test]
    pub fn test_float() {
        let mut cd:FloatConstantData = FloatConstantData::new(4);
        check(&mut cd, 1, 1, &vec![0.5, 1.0, 2.0, 3.0]);
        check(&mut cd, 5, 3, &vec![0.5, 1.0, 2.0, 3.0, 0.5, 1.0, 2.0, 3.0, 0.5, 1.0, 2.0, 3.0]);
        let cap = cd.data.capacity();
        check(&mut cd, 2, 2, &vec![0.5, 1.0, 2.0, 3.0, 0.6, 0.6, 0.6, 0.6]);
        // check that we didn't realloc
        assert_eq!(cap, cd.data.capacity());
        
        println!("cap: {}", cd.data.capacity());
    }
}
