/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::slice;
use std::ptr;
use std::cell::UnsafeCell;
use std::mem::zeroed;

use comptr::ComPtr;
use super::{FontMetrics, FontFile, DefaultDWriteRenderParams, DWriteFactory};

use winapi::um::dwrite::{DWRITE_RENDERING_MODE, DWRITE_RENDERING_MODE_DEFAULT};
use winapi::um::dwrite::{DWRITE_FONT_METRICS, DWRITE_FONT_SIMULATIONS, DWRITE_MATRIX};
use winapi::um::dwrite::{DWRITE_GLYPH_METRICS, DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC};
use winapi::um::dwrite::{IDWriteRenderingParams, IDWriteFontFace, IDWriteFontFile};
use winapi::shared::minwindef::{BOOL, FALSE};
use winapi::ctypes::c_void;
use winapi::um::dcommon::DWRITE_MEASURING_MODE;

pub struct FontFace {
    native: UnsafeCell<ComPtr<IDWriteFontFace>>,
    metrics: FontMetrics,
}

impl FontFace {
    pub fn take(native: ComPtr<IDWriteFontFace>) -> FontFace {
        unsafe {
            let mut metrics: FontMetrics = zeroed();
            let cell = UnsafeCell::new(native);
            (*cell.get()).GetMetrics(&mut metrics);
            FontFace {
                native: cell,
                metrics: metrics,
            }
        }
    }

    pub unsafe fn as_ptr(&self) -> *mut IDWriteFontFace {
        (*self.native.get()).as_ptr()
    }

    unsafe fn get_raw_files(&self) -> Vec<*mut IDWriteFontFile> {
        let mut number_of_files: u32 = 0;
        let hr = (*self.native.get()).GetFiles(&mut number_of_files, ptr::null_mut());
        assert!(hr == 0);

        let mut file_ptrs: Vec<*mut IDWriteFontFile> =
            vec![ptr::null_mut(); number_of_files as usize];
        let hr = (*self.native.get()).GetFiles(&mut number_of_files, file_ptrs.as_mut_ptr());
        assert!(hr == 0);
        file_ptrs
    }

    pub fn get_files(&self) -> Vec<FontFile> {
        unsafe {
            let file_ptrs = self.get_raw_files();
            file_ptrs.iter().map(|p| FontFile::take(ComPtr::already_addrefed(*p))).collect()
        }
    }

    pub fn create_font_face_with_simulations(&self, simulations: DWRITE_FONT_SIMULATIONS) -> FontFace {
        unsafe {
            let file_ptrs = self.get_raw_files();
            let face_type = (*self.native.get()).GetType();
            let face_index = (*self.native.get()).GetIndex();
            let mut face: ComPtr<IDWriteFontFace> = ComPtr::new();
            let hr = (*DWriteFactory()).CreateFontFace(
                face_type,
                file_ptrs.len() as u32,
                file_ptrs.as_ptr(),
                face_index,
                simulations,
                face.getter_addrefs()
            );
            for p in file_ptrs {
                let _ = ComPtr::<IDWriteFontFile>::already_addrefed(p);
            }
            assert!(hr == 0);
            FontFace::take(face)
        }
    }

    pub fn get_glyph_count(&self) -> u16 {
        unsafe {
            (*self.native.get()).GetGlyphCount()
        }
    }

    pub fn metrics(&self) -> &FontMetrics {
        &self.metrics
    }

    pub fn get_metrics(&self) -> FontMetrics {
        unsafe {
            let mut metrics: DWRITE_FONT_METRICS = zeroed();
            (*self.native.get()).GetMetrics(&mut metrics);
            metrics
        }
    }

    pub fn get_glyph_indices(&self, code_points: &[u32]) -> Vec<u16> {
        unsafe {
            let mut glyph_indices: Vec<u16> = vec![0; code_points.len()];
            let hr = (*self.native.get()).GetGlyphIndices(code_points.as_ptr(),
                                                          code_points.len() as u32,
                                                          glyph_indices.as_mut_ptr());
            assert!(hr == 0);
            glyph_indices
        }
    }

    pub fn get_design_glyph_metrics(&self, glyph_indices: &[u16], is_sideways: bool) -> Vec<DWRITE_GLYPH_METRICS> {
        unsafe {
            let mut metrics: Vec<DWRITE_GLYPH_METRICS> = vec![zeroed(); glyph_indices.len()];
            let hr = (*self.native.get()).GetDesignGlyphMetrics(glyph_indices.as_ptr(),
                                                                glyph_indices.len() as u32,
                                                                metrics.as_mut_ptr(),
                                                                is_sideways as BOOL);
            assert!(hr == 0);
            metrics
        }
    }

    pub fn get_gdi_compatible_glyph_metrics(&self, em_size: f32, pixels_per_dip: f32, transform: *const DWRITE_MATRIX,
                                            use_gdi_natural: bool, glyph_indices: &[u16], is_sideways: bool)
                                            -> Vec<DWRITE_GLYPH_METRICS>
    {
        unsafe {
            let mut metrics: Vec<DWRITE_GLYPH_METRICS> = vec![zeroed(); glyph_indices.len()];
            let hr = (*self.native.get()).GetGdiCompatibleGlyphMetrics(em_size, pixels_per_dip,
                                                                       transform,
                                                                       use_gdi_natural as BOOL,
                                                                       glyph_indices.as_ptr(),
                                                                       glyph_indices.len() as u32,
                                                                       metrics.as_mut_ptr(),
                                                                       is_sideways as BOOL);
            assert!(hr == 0);
            metrics
        }
    }

    pub fn get_font_table(&self, opentype_table_tag: u32) -> Option<Vec<u8>> {
        unsafe {
            let mut table_data_ptr: *const u8 = ptr::null_mut();
            let mut table_size: u32 = 0;
            let mut table_context: *mut c_void = ptr::null_mut();
            let mut exists: BOOL = FALSE;

            let hr = (*self.native.get()).TryGetFontTable(opentype_table_tag,
                                                          &mut table_data_ptr as *mut *const _ as *mut *const c_void,
                                                          &mut table_size,
                                                          &mut table_context,
                                                          &mut exists);
            assert!(hr == 0);

            if exists == FALSE {
                return None;
            }

            let table_bytes = slice::from_raw_parts(table_data_ptr, table_size as usize).to_vec();

            (*self.native.get()).ReleaseFontTable(table_context);

            Some(table_bytes)
        }
    }

    pub fn get_recommended_rendering_mode(&self,
                                          em_size: f32,
                                          pixels_per_dip: f32,
                                          measure_mode: DWRITE_MEASURING_MODE,
                                          rendering_params: *mut IDWriteRenderingParams) ->
                                          DWRITE_RENDERING_MODE {
      unsafe {
        let mut render_mode : DWRITE_RENDERING_MODE = DWRITE_RENDERING_MODE_DEFAULT;
        let hr = (*self.native.get()).GetRecommendedRenderingMode(em_size,
                                                                  pixels_per_dip,
                                                                  measure_mode,
                                                                  rendering_params,
                                                                  &mut render_mode);

        if !(hr != 0) {
          return DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC;
        }

        render_mode
      }
    }

    pub fn get_recommended_rendering_mode_default_params(&self,
                                                        em_size: f32,
                                                        pixels_per_dip: f32,
                                                        measure_mode: DWRITE_MEASURING_MODE) ->
                                                        DWRITE_RENDERING_MODE {
      self.get_recommended_rendering_mode(em_size,
                                          pixels_per_dip,
                                          measure_mode,
                                          DefaultDWriteRenderParams())
    }
}
