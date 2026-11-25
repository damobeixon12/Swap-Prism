import unittest
from title_display import normalize_title,title_text

class TitleEditorInput(unittest.TestCase):
    def test_free_unicode_text_is_kept_and_uppercased_for_the_game(self):
        title=normalize_title({'text':'Mi título ñ 🚗','target':'XP_Superstar'})
        self.assertEqual(title_text(title),'MI TÍTULO Ñ 🚗')
    def test_multicolor_escapes_markup_and_uses_both_endpoints(self):
        title=normalize_title({'text':'<A>','target':'real-id','multicolor':True,'color':'#ff0000','color2':'#0000ff'})
        text=title_text(title)
        self.assertIn('COLOR="#FF0000"',text);self.assertIn('COLOR="#0000FF"',text)
        self.assertIn('&#60;',text);self.assertIn('&#62;',text);self.assertNotIn('<A>',text)
        self.assertEqual(text,text.upper()) # native title renderer uppercases markup
    def test_invalid_colors_and_control_characters_are_rejected(self):
        for bad in [{'text':'a\nb','target':'id'},{'text':'a','target':'id','color':'red'},{'text':'a','target':''},{'text':'a'*49,'target':'id'}]:
            with self.assertRaises(ValueError):normalize_title(bad)
    def test_disabling_the_title_has_no_override(self):
        self.assertIsNone(normalize_title(None));self.assertIsNone(normalize_title({'enabled':False}))
    def test_single_character_gradient_has_no_division_by_zero(self):
        self.assertIn('X</FONT>',title_text(normalize_title({'text':'x','target':'id','multicolor':True})))

if __name__=='__main__':unittest.main()
