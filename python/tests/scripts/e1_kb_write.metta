!(bind! &kb (new-space))

(= (frog $x)
   (and (croaks $x)
        (eat_flies $x)))
(= (croaks Fritz) True)
(= (eat_flies Fritz) True)
(= (croaks Sam) True)
(= (eat_flies Sam) True)
(= (green $x)
   (frog $x))

(= (ift True $then) $then)
!(ift (green $x)
      (call:add_atom &kb (Green $x)))

!(assertEqualToResult
  (match &kb (Green $x) $x)
  (Fritz Sam))