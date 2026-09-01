import os
p = r"c:\Users\sakeen\Desktop\sml\rust\_out.xml"
if os.path.exists(p):
    os.remove(p)
print("cleaned" if not os.path.exists(p) else "fail")
